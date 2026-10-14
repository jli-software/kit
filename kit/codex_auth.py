"""
ChatGPT/Codex-OAuth fuer KIterminal.

kit hat keinen eigenen Login-Flow, sondern nutzt den Login von Codex CLI mit:
`codex login` legt die Tokens in ~/.codex/auth.json ab (bzw. $CODEX_HOME).
Laeuft das Access-Token ab, holt kit mit dem Refresh-Token ein neues und
schreibt es im selben Format zurueck — Codex CLI liest es von dort wieder.
"""

import base64
import json
import os
import time
from datetime import datetime, timezone
from pathlib import Path

import httpx

TOKEN_URL = "https://auth.openai.com/oauth/token"
CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann"  # oeffentliche Client-ID von Codex CLI

# Token gilt als abgelaufen, wenn es in weniger als 5 Minuten auslaeuft
EXPIRY_MARGIN_SECONDS = 300


class CodexAuthError(Exception):
    """Login fehlt oder Token konnte nicht erneuert werden."""


def auth_file() -> Path:
    codex_home = os.environ.get("CODEX_HOME")
    base = Path(codex_home) if codex_home else Path.home() / ".codex"
    return base / "auth.json"


def _jwt_exp(token: str) -> int | None:
    """Liest den exp-Claim aus einem JWT (ohne Signaturpruefung)."""
    try:
        payload = token.split(".")[1]
        payload += "=" * (-len(payload) % 4)
        return int(json.loads(base64.urlsafe_b64decode(payload))["exp"])
    except (IndexError, KeyError, ValueError, TypeError):
        return None


class CodexAuth:
    """Haelt Access-Token und Account-ID, erneuert bei Bedarf."""

    def __init__(self) -> None:
        self.path = auth_file()
        self._data = self._read()
        tokens = self._data.get("tokens") or {}
        if not tokens.get("access_token") or not tokens.get("refresh_token"):
            raise CodexAuthError(
                f"Kein ChatGPT-Login in {self.path}.\n"
                "Bitte einmal 'codex login' ausfuehren (Anmeldung mit ChatGPT)."
            )

    def _read(self) -> dict:
        try:
            return json.loads(self.path.read_text(encoding="utf-8"))
        except FileNotFoundError:
            raise CodexAuthError(
                f"{self.path} nicht gefunden.\n"
                "Codex CLI installieren und einmal 'codex login' ausfuehren."
            ) from None
        except json.JSONDecodeError as e:
            raise CodexAuthError(f"{self.path} ist kein gueltiges JSON: {e}") from None

    @property
    def tokens(self) -> dict:
        return self._data["tokens"]

    @property
    def account_id(self) -> str | None:
        return self.tokens.get("account_id")

    def access_token(self) -> str:
        """Gibt ein gueltiges Access-Token zurueck, erneuert es wenn noetig."""
        exp = _jwt_exp(self.tokens["access_token"])
        if exp is not None and exp - time.time() < EXPIRY_MARGIN_SECONDS:
            self.refresh()
        return self.tokens["access_token"]

    def refresh(self) -> None:
        """Erneuert die Tokens und schreibt sie nach auth.json zurueck."""
        # Codex CLI koennte inzwischen selbst erneuert haben → frisch lesen
        self._data = self._read()
        try:
            r = httpx.post(
                TOKEN_URL,
                json={
                    "client_id": CLIENT_ID,
                    "grant_type": "refresh_token",
                    "refresh_token": self.tokens["refresh_token"],
                    "scope": "openid profile email",
                },
                timeout=30,
            )
        except httpx.HTTPError as e:
            raise CodexAuthError(f"Token-Erneuerung fehlgeschlagen: {e}") from None

        if r.status_code != 200:
            raise CodexAuthError(
                f"Token-Erneuerung abgelehnt (HTTP {r.status_code}).\n"
                "Bitte 'codex login' erneut ausfuehren."
            )

        new = r.json()
        for key in ("access_token", "refresh_token", "id_token"):
            if new.get(key):
                self.tokens[key] = new[key]
        self._data["last_refresh"] = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        self._write()

    def _write(self) -> None:
        tmp = self.path.with_suffix(".json.kit-tmp")
        tmp.write_text(json.dumps(self._data, indent=2), encoding="utf-8")
        os.replace(tmp, self.path)
