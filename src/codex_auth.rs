//! ChatGPT/Codex-OAuth.
//!
//! kit hat keinen eigenen Login-Flow, sondern nutzt den Login von Codex CLI mit:
//! `codex login` legt die Tokens in ~/.codex/auth.json ab (bzw. $CODEX_HOME).
//! Läuft das Access-Token ab, holt kit mit dem Refresh-Token ein neues und
//! schreibt es im selben Format zurück — Codex CLI liest es von dort wieder.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};

use crate::config::home_dir;
use crate::http;

const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann"; // öffentliche Client-ID von Codex CLI

/// Token gilt als abgelaufen, wenn es in weniger als 5 Minuten ausläuft
const EXPIRY_MARGIN_SECONDS: i64 = 300;

pub fn auth_file() -> PathBuf {
    match std::env::var_os("CODEX_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir).join("auth.json"),
        _ => home_dir().join(".codex").join("auth.json"),
    }
}

/// Liest den exp-Claim aus einem JWT (ohne Signaturprüfung).
pub fn jwt_exp(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()?
        .get("exp")?
        .as_i64()
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Hält Access-Token und Account-ID, erneuert bei Bedarf.
pub struct CodexAuth {
    path: PathBuf,
    data: Value,
}

impl CodexAuth {
    pub fn load() -> Result<Self, String> {
        let path = auth_file();
        let data = read(&path)?;
        let auth = CodexAuth { path, data };
        if auth.token("access_token").is_none() || auth.token("refresh_token").is_none() {
            return Err(format!(
                "Kein ChatGPT-Login in {}.\nBitte einmal 'codex login' ausführen (Anmeldung mit ChatGPT).",
                auth.path.display()
            ));
        }
        Ok(auth)
    }

    fn token(&self, key: &str) -> Option<&str> {
        self.data
            .get("tokens")?
            .get(key)?
            .as_str()
            .filter(|s| !s.is_empty())
    }

    pub fn account_id(&self) -> Option<String> {
        self.token("account_id").map(str::to_owned)
    }

    /// Gibt ein gültiges Access-Token zurück, erneuert es wenn nötig.
    pub fn access_token(&mut self) -> Result<String, String> {
        let token = self.token("access_token").unwrap_or_default();
        if jwt_exp(token).is_some_and(|exp| exp - now() < EXPIRY_MARGIN_SECONDS) {
            self.refresh()?;
        }
        Ok(self.token("access_token").unwrap_or_default().to_owned())
    }

    /// Erneuert die Tokens und schreibt sie nach auth.json zurück.
    pub fn refresh(&mut self) -> Result<(), String> {
        // Codex CLI könnte inzwischen selbst erneuert haben → frisch lesen
        self.data = read(&self.path)?;
        let refresh_token = self.token("refresh_token").unwrap_or_default().to_owned();

        let resp = http::agent().post(TOKEN_URL).send_json(json!({
            "client_id": CLIENT_ID,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "scope": "openid profile email",
        }));
        let new: Value = match resp {
            Ok(r) => r
                .into_json()
                .map_err(|e| format!("Token-Antwort unlesbar: {e}"))?,
            Err(ureq::Error::Status(code, _)) => {
                return Err(format!(
                    "Token-Erneuerung abgelehnt (HTTP {code}).\nBitte 'codex login' erneut ausführen."
                ));
            }
            Err(e) => return Err(format!("Token-Erneuerung fehlgeschlagen: {e}")),
        };

        let tokens = self
            .data
            .get_mut("tokens")
            .and_then(Value::as_object_mut)
            .ok_or("auth.json ohne tokens")?;
        for key in ["access_token", "refresh_token", "id_token"] {
            if let Some(v) = new
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                tokens.insert(key.into(), Value::String(v.into()));
            }
        }
        let stamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        self.data["last_refresh"] = Value::String(stamp);
        self.write()
    }

    fn write(&self) -> Result<(), String> {
        let tmp = self.path.with_extension("json.kit-tmp");
        let text = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;
        fs::write(&tmp, text).map_err(|e| format!("{} nicht schreibbar: {e}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .map_err(|e| format!("{} nicht schreibbar: {e}", self.path.display()))
    }
}

fn read(path: &PathBuf) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|_| {
        format!(
            "{} nicht gefunden.\nCodex CLI installieren und einmal 'codex login' ausführen.",
            path.display()
        )
    })?;
    serde_json::from_str(&text)
        .map_err(|e| format!("{} ist kein gültiges JSON: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn jwt(exp: i64) -> String {
        let payload = URL_SAFE_NO_PAD.encode(format!(r#"{{"exp": {exp}}}"#));
        format!("header.{payload}.signature")
    }

    #[test]
    fn reads_exp() {
        assert_eq!(jwt_exp(&jwt(1234567890)), Some(1234567890));
        assert_eq!(jwt_exp("kein-jwt"), None);
    }

    #[test]
    fn valid_token_needs_no_refresh() {
        let token = jwt(now() + 3600);
        let mut auth = CodexAuth {
            path: PathBuf::from("gibt-es-nicht.json"), // refresh() würde hier scheitern
            data: json!({"tokens": {"access_token": token, "refresh_token": "r", "account_id": "acc"}}),
        };
        assert_eq!(auth.access_token().unwrap(), token);
        assert_eq!(auth.account_id().as_deref(), Some("acc"));
    }

    #[test]
    fn missing_file_hints_login() {
        let err = read(&PathBuf::from("gibt/es/nicht/auth.json")).unwrap_err();
        assert!(err.contains("codex login"));
    }
}
