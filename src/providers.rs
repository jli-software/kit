//! Provider: alle Antworten kommen gestreamt als Text-Stücke.
//!
//! - codex:     ChatGPT-Abo via Codex-OAuth (Login von Codex CLI),
//!   Responses-API auf chatgpt.com/backend-api/codex
//! - anthropic: Anthropic Messages-API
//! - deepseek:  DeepSeek über den Anthropic-kompatiblen Endpoint

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::codex_auth::CodexAuth;
use crate::config::ModeConfig;
use crate::http::{self, read_sse};

const CODEX_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

/// Effort-Stufen, die das Codex-Backend akzeptiert ("none" → kleinste Stufe)
const CODEX_EFFORTS: [&str; 3] = ["low", "medium", "high"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Message {
            role: "user".into(),
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Message {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug)]
pub enum ApiError {
    /// Login/API-Key fehlt oder Provider unbekannt → kit beendet sich
    Setup(String),
    /// Fehler beim Call → Meldung, Chat läuft weiter
    Call(String),
}

pub fn display_name(provider: &str) -> &str {
    match provider {
        "codex" => "OpenAI Codex",
        "anthropic" => "Anthropic",
        "deepseek" => "DeepSeek",
        other => other,
    }
}

/// Startet einen Call und gibt jedes Text-Stück an `on_text` weiter.
pub fn stream_text(
    mode: &ModeConfig,
    system: &str,
    messages: &[Message],
    on_text: &mut dyn FnMut(&str),
) -> Result<(), ApiError> {
    match mode.provider.as_str() {
        "codex" => codex_stream(mode, system, messages, on_text),
        "anthropic" => anthropic_stream(
            "https://api.anthropic.com",
            "ANTHROPIC_API_KEY",
            mode,
            system,
            messages,
            on_text,
        ),
        "deepseek" => anthropic_stream(
            "https://api.deepseek.com/anthropic",
            "DEEPSEEK_API_KEY",
            mode,
            system,
            messages,
            on_text,
        ),
        other => Err(ApiError::Setup(format!(
            "Unbekannter Provider '{other}'. Verfügbar: codex, anthropic, deepseek"
        ))),
    }
}

// ── Codex (Responses-API) ─────────────────────────────────────────────────

/// Wandelt die Chat-History ins Responses-Input-Format um.
pub fn codex_input(messages: &[Message]) -> Value {
    messages
        .iter()
        .map(|m| {
            let part = if m.role == "assistant" { "output_text" } else { "input_text" };
            json!({"type": "message", "role": m.role, "content": [{"type": part, "text": m.content}]})
        })
        .collect()
}

pub fn codex_body(model: &str, system: &str, messages: &[Message], effort: &str) -> Value {
    let effort = if CODEX_EFFORTS.contains(&effort) {
        effort
    } else {
        "low"
    };
    json!({
        "model": model,
        "instructions": system,
        "input": codex_input(messages),
        "tools": [],
        "tool_choice": "auto",
        "parallel_tool_calls": false,
        "reasoning": {"effort": effort},
        "store": false,
        "stream": true,
        "include": [],
    })
}

/// Verarbeitet ein Codex-Event; `Ok(false)` = Antwort fertig.
pub fn codex_event(event: &Value, on_text: &mut dyn FnMut(&str)) -> Result<bool, String> {
    match event.get("type").and_then(Value::as_str).unwrap_or("") {
        "response.output_text.delta" => {
            on_text(event.get("delta").and_then(Value::as_str).unwrap_or(""));
            Ok(true)
        }
        "response.failed" | "error" => {
            let msg = event
                .get("response")
                .and_then(http::error_message)
                .or_else(|| http::error_message(event))
                .unwrap_or_else(|| event.to_string());
            Err(format!("Codex: {msg}"))
        }
        "response.completed" => Ok(false),
        _ => Ok(true),
    }
}

fn codex_status_error(code: u16, resp: ureq::Response) -> String {
    let body: Value = resp.into_json().unwrap_or(Value::Null);
    if code == 429 {
        let resets = body
            .get("error")
            .and_then(|e| e.get("resets_in_seconds"))
            .and_then(Value::as_i64);
        return match resets {
            Some(s) => format!(
                "ChatGPT-Nutzungslimit erreicht — wieder frei in ca. {} Min.",
                s / 60
            ),
            None => "ChatGPT-Nutzungslimit erreicht".into(),
        };
    }
    let msg = http::error_message(&body).unwrap_or_else(|| body.to_string());
    format!("Codex HTTP {code}: {msg}")
}

fn codex_stream(
    mode: &ModeConfig,
    system: &str,
    messages: &[Message],
    on_text: &mut dyn FnMut(&str),
) -> Result<(), ApiError> {
    let mut auth = CodexAuth::load().map_err(ApiError::Setup)?;
    let body = codex_body(&mode.model, system, messages, &mode.reasoning_effort);
    let session_id = uuid::Uuid::new_v4().to_string();

    for attempt in 0..2 {
        let token = auth.access_token().map_err(ApiError::Call)?;
        let mut req = http::agent()
            .post(CODEX_URL)
            .set("Authorization", &format!("Bearer {token}"))
            .set("OpenAI-Beta", "responses=experimental")
            .set("originator", "codex_cli_rs")
            .set("session_id", &session_id)
            .set("accept", "text/event-stream");
        if let Some(account) = auth.account_id() {
            req = req.set("chatgpt-account-id", &account);
        }

        match req.send_json(&body) {
            Ok(resp) => {
                let reader = std::io::BufReader::new(resp.into_reader());
                return read_sse(reader, |ev| codex_event(ev, on_text)).map_err(ApiError::Call);
            }
            // Token serverseitig ungültig → einmal erneuern
            Err(ureq::Error::Status(401, _)) if attempt == 0 => {
                auth.refresh().map_err(ApiError::Call)?
            }
            Err(ureq::Error::Status(code, resp)) => {
                return Err(ApiError::Call(codex_status_error(code, resp)));
            }
            Err(e) => return Err(ApiError::Call(format!("Keine Verbindung zu Codex: {e}"))),
        }
    }
    unreachable!("zweiter Versuch endet immer mit return")
}

// ── Anthropic / DeepSeek (Messages-API) ───────────────────────────────────

pub fn anthropic_body(mode: &ModeConfig, system: &str, messages: &[Message]) -> Value {
    let mut body = json!({
        "model": mode.model,
        "max_tokens": mode.max_tokens,
        "system": system,
        "messages": messages,
        "stream": true,
    });
    if CODEX_EFFORTS.contains(&mode.reasoning_effort.as_str()) {
        body["thinking"] = json!({"type": "adaptive"});
        body["output_config"] = json!({"effort": mode.reasoning_effort});
    }
    body
}

/// Verarbeitet ein Messages-API-Event; `Ok(false)` = Antwort fertig.
pub fn anthropic_event(event: &Value, on_text: &mut dyn FnMut(&str)) -> Result<bool, String> {
    match event.get("type").and_then(Value::as_str).unwrap_or("") {
        "content_block_delta" => {
            if let Some(text) = event
                .get("delta")
                .filter(|d| d["type"] == "text_delta")
                .and_then(|d| d["text"].as_str())
            {
                on_text(text);
            }
            Ok(true)
        }
        "error" => Err(http::error_message(event).unwrap_or_else(|| event.to_string())),
        "message_stop" => Ok(false),
        _ => Ok(true),
    }
}

fn anthropic_stream(
    base_url: &str,
    env: &str,
    mode: &ModeConfig,
    system: &str,
    messages: &[Message],
    on_text: &mut dyn FnMut(&str),
) -> Result<(), ApiError> {
    let name = display_name(&mode.provider);
    let key = std::env::var(env).ok().filter(|k| !k.is_empty()).ok_or_else(|| {
        ApiError::Setup(format!(
            "{name} API-Key nicht gesetzt!\nBitte setze die Umgebungsvariable: export {env}='dein-key'"
        ))
    })?;

    let resp = http::agent()
        .post(&format!("{base_url}/v1/messages"))
        .set("x-api-key", &key)
        .set("anthropic-version", "2023-06-01")
        .send_json(anthropic_body(mode, system, messages));

    match resp {
        Ok(resp) => {
            let reader = std::io::BufReader::new(resp.into_reader());
            read_sse(reader, |ev| anthropic_event(ev, on_text)).map_err(ApiError::Call)
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body: Value = resp.into_json().unwrap_or(Value::Null);
            let msg = http::error_message(&body).unwrap_or_default();
            Err(ApiError::Call(match code {
                401 => format!("Ungültiger API-Key! Prüfe {env}"),
                429 => "Rate-Limit erreicht – warte einen Moment".into(),
                _ => format!("API-Fehler {code}: {msg}"),
            }))
        }
        Err(e) => Err(ApiError::Call(format!("Keine Verbindung zu {name}: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type EventFn = fn(&Value, &mut dyn FnMut(&str)) -> Result<bool, String>;

    fn collect(events: &[Value], f: EventFn) -> Result<String, String> {
        let mut out = String::new();
        for ev in events {
            if !f(ev, &mut |t| out.push_str(t))? {
                break;
            }
        }
        Ok(out)
    }

    #[test]
    fn codex_input_maps_roles() {
        let items = codex_input(&[Message::user("Hallo"), Message::assistant("Hi")]);
        assert_eq!(
            items[0]["content"][0],
            json!({"type": "input_text", "text": "Hallo"})
        );
        assert_eq!(items[1]["role"], "assistant");
        assert_eq!(
            items[1]["content"][0],
            json!({"type": "output_text", "text": "Hi"})
        );
    }

    #[test]
    fn codex_body_effort() {
        for (effort, expected) in [("none", "low"), ("low", "low"), ("high", "high")] {
            let body = codex_body("gpt-6-luna", "sys", &[Message::user("x")], effort);
            assert_eq!(body["reasoning"]["effort"], expected);
            assert_eq!(body["instructions"], "sys");
            assert_eq!(body["store"], false);
        }
    }

    #[test]
    fn codex_events_until_completed() {
        let events = [
            json!({"type": "response.created"}),
            json!({"type": "response.output_text.delta", "delta": "Hallo "}),
            json!({"type": "response.output_text.delta", "delta": "Welt"}),
            json!({"type": "response.completed"}),
            json!({"type": "response.output_text.delta", "delta": "zu spät"}),
        ];
        assert_eq!(collect(&events, codex_event).unwrap(), "Hallo Welt");
    }

    #[test]
    fn codex_failure_is_error() {
        let events =
            [json!({"type": "response.failed", "response": {"error": {"message": "kaputt"}}})];
        assert!(
            collect(&events, codex_event)
                .unwrap_err()
                .contains("kaputt")
        );
    }

    #[test]
    fn anthropic_events() {
        let events = [
            json!({"type": "message_start"}),
            json!({"type": "content_block_delta", "delta": {"type": "thinking_delta", "thinking": "hm"}}),
            json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "Hallo"}}),
            json!({"type": "message_stop"}),
        ];
        assert_eq!(collect(&events, anthropic_event).unwrap(), "Hallo");
    }

    #[test]
    fn sse_reader_parses_data_lines() {
        let raw = "event: x\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"A\"}\n\ndata: [DONE]\n";
        let mut out = String::new();
        read_sse(raw.as_bytes(), |ev| {
            codex_event(ev, &mut |t| out.push_str(t))
        })
        .unwrap();
        assert_eq!(out, "A");
    }
}
