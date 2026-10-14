"""
Multi-Provider Client für KIterminal.

- anthropic: Anthropic Messages API (nativ)
- deepseek:  DeepSeek via Anthropic-kompatiblem Endpoint
             https://api-docs.deepseek.com/guides/anthropic_api
- codex:     ChatGPT-Abo via Codex-OAuth (Login von Codex CLI),
             Responses-API auf chatgpt.com/backend-api/codex
"""

import json
import os
import uuid
from collections.abc import Iterator

import anthropic
import httpx

from kit.codex_auth import CodexAuth, CodexAuthError

# Provider-Konfiguration
PROVIDERS = {
    "anthropic": {
        "name": "Anthropic",
        "env": "ANTHROPIC_API_KEY",
        "base_url": None,  # Standard-URL von anthropic SDK
    },
    "deepseek": {
        "name": "DeepSeek",
        "env": "DEEPSEEK_API_KEY",
        "base_url": "https://api.deepseek.com/anthropic",
    },
    "codex": {
        "name": "OpenAI Codex",
        "env": None,  # OAuth statt API-Key
        "base_url": "https://chatgpt.com/backend-api/codex",
    },
}

# Effort-Stufen, die das Codex-Backend akzeptiert ("none" → kleinste Stufe)
CODEX_EFFORTS = ("low", "medium", "high")


class ProviderError(Exception):
    """Fehler eines Providers ohne Anthropic-SDK (z.B. Codex)."""


# Alle Fehler, die ein API-Call werfen kann
API_ERRORS = (anthropic.APIError, ProviderError, CodexAuthError)


def get_client(provider_name: str) -> anthropic.Anthropic:
    """
    Erstellt einen Anthropic-Client für den angegebenen Provider.

    Args:
        provider_name: "anthropic" oder "deepseek"

    Returns:
        anthropic.Anthropic Client-Instanz

    Raises:
        ValueError: Wenn der Provider unbekannt ist oder der API-Key fehlt
    """
    if provider_name not in PROVIDERS or not PROVIDERS[provider_name]["env"]:
        raise ValueError(
            f"Unbekannter Provider '{provider_name}'. "
            f"Verfügbar: {', '.join(PROVIDERS.keys())}"
        )

    cfg = PROVIDERS[provider_name]
    api_key = os.environ.get(cfg["env"])

    if not api_key:
        raise ValueError(
            f"{cfg['name']} API-Key nicht gesetzt!\n"
            f"Bitte setze die Umgebungsvariable: export {cfg['env']}='dein-key'"
        )

    kwargs = {"api_key": api_key}
    if cfg["base_url"]:
        kwargs["base_url"] = cfg["base_url"]

    return anthropic.Anthropic(**kwargs)


def get_provider_name(provider_name: str) -> str:
    """Gibt den lesbaren Namen eines Providers zurück."""
    return PROVIDERS.get(provider_name, {}).get("name", provider_name)


def list_providers() -> list[str]:
    """Gibt alle verfügbaren Provider-Namen zurück."""
    return list(PROVIDERS.keys())


# ── Einheitlicher Stream ──────────────────────────────────────────────────

def stream_text(provider: str, *, model: str, system: str, messages: list[dict],
                max_tokens: int = 4096, reasoning_effort: str = "none") -> Iterator[str]:
    """
    Startet einen Call und liefert die Antwort als Text-Stücke.

    Login/API-Key werden sofort geprüft (ValueError), nicht erst beim
    ersten Stück — damit Konfigurationsfehler vor dem Live-Display kommen.
    """
    if provider == "codex":
        try:
            auth = CodexAuth()
        except CodexAuthError as e:
            raise ValueError(str(e)) from None
        return _codex_stream(auth, model, system, messages, reasoning_effort)

    client = get_client(provider)
    params = dict(model=model, max_tokens=max_tokens, system=system, messages=messages)
    if reasoning_effort in ("low", "medium", "high"):
        params["thinking"] = {"type": "adaptive"}
        params["output_config"] = {"effort": reasoning_effort}
    return _anthropic_stream(client, params)


def _anthropic_stream(client: anthropic.Anthropic, params: dict) -> Iterator[str]:
    with client.messages.stream(**params) as s:
        yield from s.text_stream


# ── Codex (Responses-API) ─────────────────────────────────────────────────

def codex_input(messages: list[dict]) -> list[dict]:
    """Wandelt die Chat-History (Messages-Format) ins Responses-Input-Format."""
    items = []
    for msg in messages:
        role = msg["role"]
        part_type = "output_text" if role == "assistant" else "input_text"
        items.append({
            "type": "message",
            "role": role,
            "content": [{"type": part_type, "text": msg["content"]}],
        })
    return items


def codex_body(model: str, system: str, messages: list[dict], reasoning_effort: str) -> dict:
    effort = reasoning_effort if reasoning_effort in CODEX_EFFORTS else "low"
    return {
        "model": model,
        "instructions": system,
        "input": codex_input(messages),
        "tools": [],
        "tool_choice": "auto",
        "parallel_tool_calls": False,
        "reasoning": {"effort": effort},
        "store": False,
        "stream": True,
        "include": [],
    }


def parse_sse(lines: Iterator[str]) -> Iterator[str]:
    """Liest SSE-Zeilen des Codex-Backends und liefert die Text-Deltas."""
    for line in lines:
        if not line.startswith("data:"):
            continue
        data = line[5:].strip()
        if not data or data == "[DONE]":
            continue
        try:
            event = json.loads(data)
        except json.JSONDecodeError:
            continue
        etype = event.get("type", "")
        if etype == "response.output_text.delta":
            yield event.get("delta", "")
        elif etype in ("response.failed", "error"):
            err = (event.get("response") or {}).get("error") or event.get("error") or event
            message = err.get("message") if isinstance(err, dict) else str(err)
            raise ProviderError(f"Codex: {message}")
        elif etype == "response.completed":
            return


def _codex_error(r: httpx.Response) -> ProviderError:
    try:
        r.read()
        payload = r.json()
        err = payload.get("error") or payload.get("detail") or {}
    except (ValueError, AttributeError, httpx.HTTPError):
        err = {}
    message = err.get("message") if isinstance(err, dict) else str(err)
    if r.status_code == 429:
        resets = err.get("resets_in_seconds") if isinstance(err, dict) else None
        hint = f" — wieder frei in ca. {int(resets) // 60} Min." if resets else ""
        return ProviderError(f"ChatGPT-Nutzungslimit erreicht{hint}")
    return ProviderError(f"Codex HTTP {r.status_code}: {message or r.text[:300]}")


def _codex_stream(auth: CodexAuth, model: str, system: str, messages: list[dict],
                  reasoning_effort: str) -> Iterator[str]:
    url = PROVIDERS["codex"]["base_url"] + "/responses"
    body = codex_body(model, system, messages, reasoning_effort)
    session_id = str(uuid.uuid4())

    for attempt in range(2):
        headers = {
            "Authorization": f"Bearer {auth.access_token()}",
            "OpenAI-Beta": "responses=experimental",
            "originator": "codex_cli_rs",
            "session_id": session_id,
            "accept": "text/event-stream",
            "content-type": "application/json",
        }
        if auth.account_id:
            headers["chatgpt-account-id"] = auth.account_id

        try:
            with httpx.stream("POST", url, headers=headers, json=body,
                              timeout=httpx.Timeout(30, read=300)) as r:
                if r.status_code == 401 and attempt == 0:
                    auth.refresh()  # Token serverseitig ungueltig → einmal erneuern
                    continue
                if r.status_code != 200:
                    raise _codex_error(r)
                yield from parse_sse(r.iter_lines())
                return
        except httpx.HTTPError as e:
            raise ProviderError(f"Keine Verbindung zu Codex: {e}") from None
