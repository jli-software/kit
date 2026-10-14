"""Offline-Tests für den Codex-Provider (kein Netzwerk, kein Login nötig)."""

import base64
import json
import time

import pytest

from kit.codex_auth import CodexAuth, CodexAuthError, _jwt_exp
from kit.providers import ProviderError, codex_body, codex_input, parse_sse


def _jwt(exp: int) -> str:
    payload = base64.urlsafe_b64encode(json.dumps({"exp": exp}).encode()).decode().rstrip("=")
    return f"header.{payload}.signature"


def test_codex_input_maps_roles():
    items = codex_input([
        {"role": "user", "content": "Hallo"},
        {"role": "assistant", "content": "Hi"},
    ])
    assert items[0]["content"] == [{"type": "input_text", "text": "Hallo"}]
    assert items[1]["role"] == "assistant"
    assert items[1]["content"] == [{"type": "output_text", "text": "Hi"}]


@pytest.mark.parametrize("effort,expected", [("none", "low"), ("low", "low"), ("high", "high")])
def test_codex_body_effort(effort, expected):
    body = codex_body("gpt-6-luna", "sys", [{"role": "user", "content": "x"}], effort)
    assert body["reasoning"] == {"effort": expected}
    assert body["instructions"] == "sys"
    assert body["stream"] is True and body["store"] is False


def test_parse_sse_yields_deltas_until_completed():
    lines = [
        "event: response.created",
        'data: {"type": "response.created"}',
        "",
        'data: {"type": "response.output_text.delta", "delta": "Hallo "}',
        'data: {"type": "response.output_text.delta", "delta": "Welt"}',
        'data: {"type": "response.completed"}',
        'data: {"type": "response.output_text.delta", "delta": "zu spät"}',
    ]
    assert "".join(parse_sse(iter(lines))) == "Hallo Welt"


def test_parse_sse_raises_on_failure():
    lines = ['data: {"type": "response.failed", "response": {"error": {"message": "kaputt"}}}']
    with pytest.raises(ProviderError, match="kaputt"):
        list(parse_sse(iter(lines)))


def test_jwt_exp():
    assert _jwt_exp(_jwt(1234567890)) == 1234567890
    assert _jwt_exp("kein-jwt") is None


def test_auth_missing_file(tmp_path, monkeypatch):
    monkeypatch.setenv("CODEX_HOME", str(tmp_path))
    with pytest.raises(CodexAuthError, match="codex login"):
        CodexAuth()


def test_auth_valid_token_needs_no_refresh(tmp_path, monkeypatch):
    monkeypatch.setenv("CODEX_HOME", str(tmp_path))
    token = _jwt(int(time.time()) + 3600)
    (tmp_path / "auth.json").write_text(json.dumps({
        "tokens": {"access_token": token, "refresh_token": "r", "account_id": "acc"},
    }))
    auth = CodexAuth()
    monkeypatch.setattr(auth, "refresh", lambda: pytest.fail("darf nicht erneuern"))
    assert auth.access_token() == token
    assert auth.account_id == "acc"
