//! Gemeinsamer HTTP-Agent und SSE-Leser.

use std::io::BufRead;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;

use crate::interrupt;

pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .try_proxy_from_env(true)
            .timeout_connect(Duration::from_secs(30))
            .timeout_read(Duration::from_secs(300))
            .user_agent(concat!("kit/", env!("CARGO_PKG_VERSION")))
            .build()
    })
}

/// Liest Server-Sent Events und ruft `on_event` mit jedem JSON-Datenblock auf.
/// `on_event` gibt `false` zurück, wenn der Stream fertig ist.
/// Bricht ab, sobald der Benutzer Ctrl+C drückt.
pub fn read_sse(
    reader: impl BufRead,
    mut on_event: impl FnMut(&Value) -> Result<bool, String>,
) -> Result<(), String> {
    for line in reader.lines() {
        if interrupt::requested() {
            return Ok(());
        }
        let line = line.map_err(|e| format!("Verbindung abgebrochen: {e}"))?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if !on_event(&event)? {
            return Ok(());
        }
    }
    Ok(())
}

/// Fehlermeldung aus einem JSON-Fehlerkörper ziehen (`error.message`, `detail` …).
pub fn error_message(body: &Value) -> Option<String> {
    let err = body
        .get("error")
        .or_else(|| body.get("detail"))
        .unwrap_or(body);
    match err {
        Value::String(s) => Some(s.clone()),
        Value::Object(_) => err
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}
