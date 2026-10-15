//! Konfiguration: ~/.config/KIterminal/config.json und Instruction-Dateien.
//!
//! Format v2.0 — pro Modus: provider + model + optionale Einstellungen.
//! Dateien und Format sind dieselben wie in der Python-Version (≤ 0.10).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODES: [&str; 6] = [
    "normalchat",
    "codex",
    "email",
    "email_advanced",
    "email_pro",
    "translate",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModeConfig {
    pub provider: String,
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_effort")]
    pub reasoning_effort: String,
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_effort() -> String {
    "none".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: String,
    pub language: String,
    pub modes: BTreeMap<String, ModeConfig>,
    /// Leer = Default-Pfad ~/.config/KIterminal/KITchats/
    #[serde(default, alias = "Chatpath")]
    pub chat_path: String,
}

pub fn home_dir() -> PathBuf {
    #[allow(deprecated)]
    std::env::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    home_dir().join(".config").join("KIterminal")
}

pub fn instructions_dir() -> PathBuf {
    config_dir().join("instructions")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

/// Default: ChatGPT-Abo via Codex-Login, kleinstes Modell
fn default_mode(mode: &str) -> ModeConfig {
    let (max_tokens, effort) = match mode {
        "codex" => (4096, "medium"),
        "email" | "translate" => (2048, "low"),
        _ => (4096, "low"),
    };
    ModeConfig {
        provider: "codex".into(),
        model: "gpt-6-luna".into(),
        max_tokens,
        reasoning_effort: effort.into(),
    }
}

pub fn default_config() -> Config {
    Config {
        version: "2.0".into(),
        language: "de".into(),
        modes: MODES
            .iter()
            .map(|m| (m.to_string(), default_mode(m)))
            .collect(),
        chat_path: String::new(),
    }
}

impl Config {
    /// Modus-Konfiguration; fehlt der Modus in der Datei, gilt der Default.
    pub fn mode(&self, mode: &str) -> ModeConfig {
        self.modes
            .get(mode)
            .cloned()
            .unwrap_or_else(|| default_mode(mode))
    }
}

/// Migriert Config v1.x → v2.0.
///
/// v1 hatte flache Keys wie "normalchat": "claude-sonnet-4-6",
/// v2 hat "modes": {"normalchat": {"provider": "...", "model": "..."}}.
pub fn migrate_v1(old: &Value) -> Config {
    let mut cfg = default_config();
    for mode in MODES {
        if let Some(model) = old.get(mode).and_then(Value::as_str) {
            // v1 war immer Anthropic; fehlende Modi behalten den v2-Default
            let entry = cfg.modes.get_mut(mode).expect("Default-Modus");
            entry.provider = "anthropic".into();
            entry.model = model.into();
        }
    }
    if let Some(lang) = old.get("language").and_then(Value::as_str) {
        cfg.language = lang.into();
    }
    if let Some(path) = old.get("Chatpath").and_then(Value::as_str) {
        cfg.chat_path = path.into();
    }
    cfg
}

fn write_config(cfg: &Config) -> Result<(), String> {
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(config_file(), json + "\n")
        .map_err(|e| format!("{} nicht schreibbar: {e}", config_file().display()))
}

/// Legt Ordner, Default-Config und Default-Instructions an, falls sie fehlen.
pub fn ensure_files() -> Result<(), String> {
    fs::create_dir_all(instructions_dir())
        .map_err(|e| format!("{} nicht anlegbar: {e}", instructions_dir().display()))?;
    for (file, content) in DEFAULT_INSTRUCTIONS {
        let path = instructions_dir().join(file);
        if !path.exists() {
            let _ = fs::write(path, content);
        }
    }
    if !config_file().exists() {
        write_config(&default_config())?;
    }
    Ok(())
}

pub fn load() -> Result<Config, String> {
    ensure_files()?;
    let text = fs::read_to_string(config_file())
        .map_err(|e| format!("{} nicht lesbar: {e}", config_file().display()))?;
    let raw: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} ist kein gültiges JSON: {e}", config_file().display()))?;

    if raw.get("version").and_then(Value::as_str) != Some("2.0") || raw.get("modes").is_none() {
        let cfg = migrate_v1(&raw);
        write_config(&cfg)?;
        return Ok(cfg);
    }
    serde_json::from_value(raw).map_err(|e| format!("{}: {e}", config_file().display()))
}

/// Setzt Config und Instructions auf Werkseinstellungen zurück.
pub fn reset() -> Result<(), String> {
    fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
    write_config(&default_config())?;
    ensure_files()
}

fn language_prefix(language: &str) -> &'static str {
    match language.to_lowercase().as_str() {
        "en" => "IMPORTANT: Always respond in English.\n\n",
        "de" => "IMPORTANT: Always respond in German.\n\n",
        "chde" => "IMPORTANT: Always respond in Swiss German.\n\n",
        "fr" => "IMPORTANT: Always respond in French.\n\n",
        "es" => "IMPORTANT: Always respond in Spanish.\n\n",
        "it" => "IMPORTANT: Always respond in Italian.\n\n",
        "pt" => "IMPORTANT: Always respond in Portuguese.\n\n",
        "nl" => "IMPORTANT: Always respond in Dutch.\n\n",
        _ => "",
    }
}

/// Lädt die Instructions eines Modus mit Sprach-Präfix (leer, wenn die Datei fehlt).
pub fn instruction(mode: &str, language: &str) -> String {
    let path = instructions_dir().join(format!("{mode}.txt"));
    match fs::read_to_string(path) {
        Ok(text) => format!("{}{}", language_prefix(language), text),
        Err(_) => String::new(),
    }
}

pub const DEFAULT_INSTRUCTIONS: [(&str, &str); 6] = [
    ("normalchat.txt", NORMALCHAT),
    ("codex.txt", CODEX),
    ("email.txt", EMAIL),
    ("email_advanced.txt", EMAIL_ADVANCED),
    ("email_pro.txt", EMAIL_PRO),
    ("translate.txt", TRANSLATE),
];

const NORMALCHAT: &str = r#"Always respond in Markdown format.

## Completeness & Persistence
- Answer the question completely END-TO-END before stopping.
- No unnecessary follow-up questions – make reasonable assumptions and document them at the end under 'Assumptions: …'.
- When uncertain: mark briefly, but don't block.
- Bias for action: If unclear → reasonable default assumption + justification.

## Output Length & Structure
- **Simple questions:** 2-4 sentences, no headings.
- **Medium complexity:** 4-6 bullet points OR 6-8 sentences. Maximum 1 heading.
- **Complex/multi-part questions:** Structured with **headings** (1-3 words, bold), bullet points allowed.
- Bullet points only for steps/options/lists – not for everything.
- No nested lists. No ANSI codes.

## Tone & Style
- **Efficiency shows respect:** Directly to the solution, no filler words.
- Avoid: 'Sure!', 'Of course!', 'Got it!', 'Thanks for asking!' – start immediately with the answer.
- Friendly but concise. No platitudes. Active voice.
- Code/commands/paths in `backticks`. Never combine backticks with **.

## Structure Guidelines
- Headings: Optional, only when complexity justifies it. **Bold**, 1-3 words.
- Bullet points: Use `-`. Combine related points. One line when possible.
- Order: General → Specific → Supporting.
- Code samples in fenced code blocks with language hint.

## Conclusion
- End with concrete result or next step (when appropriate).
- No explicit confirmation questions like 'Does that work?' or 'Is that enough?'.
"#;

const CODEX: &str = r#"You are Codex, a coding assistant. You are running as a coding agent in the KIterminal CLI on a user's computer.

Always respond in Markdown format.

## System Context
- Operating System: {currentOS}
- Platform: {platforminfo}

## General
- The arguments to `shell` will be passed to execvp(). Most terminal commands should be prefixed with ["bash", "-lc"].
- Always set the `workdir` param when using the shell function. Do not use `cd` unless absolutely necessary.
- When searching for text or files, prefer using `rg` or `rg --files` respectively because `rg` is much faster than alternatives like `grep`. (If the `rg` command is not found, then use alternatives.)

## Editing constraints
- Default to ASCII when editing or creating files. Only introduce non-ASCII or other Unicode characters when there is a clear justification and the file already uses them.
- Add succinct code comments that explain what is going on if code is not self-explanatory.
- You may be in a dirty git worktree.
    * NEVER revert existing changes you did not make unless explicitly requested.
    * If asked to make a commit or code edits and there are unrelated changes, don't revert those changes.
    * If the changes are in files you've touched recently, read carefully and understand how you can work with the changes rather than reverting them.
    * If the changes are in unrelated files, just ignore them and don't revert them.
- While you are working, you might notice unexpected changes. If this happens, STOP IMMEDIATELY and ask the user how they would like to proceed.

## Special user requests
- If the user makes a simple request (such as asking for the time) which you can fulfill by running a terminal command (such as `date`), you should do so.
- If the user asks for a "review", default to a code review mindset: prioritise identifying bugs, risks, behavioural regressions, and missing tests.

## Presenting your work
- Default: be very concise; friendly coding teammate tone.
- Ask only when needed; suggest ideas; mirror the user's style.
- For substantial work, summarize clearly.
- Skip heavy formatting for simple confirmations.
- Don't dump large files you've written; reference paths only.
- Offer logical next steps (tests, commits, build) briefly.
- For code changes: lead with a quick explanation, then more details on where and why.
- When suggesting multiple options, use numeric lists so the user can quickly respond.

### Final answer structure
- Plain text; CLI handles styling. Use structure only when it helps scanability.
- Headers: optional; short Title Case (1-3 words) wrapped in **…**.
- Bullets: use - ; merge related points; keep to one line when possible.
- Monospace: backticks for commands/paths/env vars/code ids.
- Code samples in fenced code blocks; add a language hint whenever obvious.
- Structure: group related bullets; order sections general → specific → supporting.
- Tone: collaborative, concise, factual; present tense, active voice.
- No nested bullets/hierarchies; no ANSI codes.
- File References: use inline code to make file paths clickable; include line numbers."#;

const EMAIL: &str = "Output ONLY the corrected email text — no comments, no explanations, \
no correction lists, no headings, no extra formatting. Just the improved text, nothing else.\n\n\
Improve only style, spelling, punctuation, and clarity. \
Keep the original voice and tone. Do not add or remove content.";

const EMAIL_ADVANCED: &str = "Output ONLY the improved email text — no comments, no explanations, \
no correction lists, no headings, no extra formatting. Just the improved text, nothing else.\n\n\
Improve style, spelling, punctuation, clarity, and content. \
Strengthen weak phrasing, fix logical flow, remove redundancies, and sharpen the message. \
Keep the original intent, voice, and all facts. Do not add or remove relevant content.";

const EMAIL_PRO: &str = "Output ONLY the rewritten email text — no comments, no explanations, \
no headings, no extra formatting. Just the improved text, nothing else.\n\n\
Rewrite the email in a professional business tone. Use formal but clear language, \
logical structure, and concise phrasing. Add a clear call to action where appropriate. \
Keep the original intent and all facts. Eliminate informal phrasing, typos, and weak \
formulations. The result should read as polished business communication.";

const TRANSLATE: &str = "Always respond in Markdown format. \
Translate the text between German and English (auto-detect source language). \
Provide only the translated text, without explanations or additional content.";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn migrate_v1_keeps_models_as_anthropic() {
        let old = json!({"normalchat": "claude-haiku-4-5", "language": "en", "Chatpath": "/x"});
        let cfg = migrate_v1(&old);
        assert_eq!(cfg.mode("normalchat").provider, "anthropic");
        assert_eq!(cfg.mode("normalchat").model, "claude-haiku-4-5");
        assert_eq!(cfg.mode("email").provider, "codex");
        assert_eq!(cfg.language, "en");
        assert_eq!(cfg.chat_path, "/x");
    }

    #[test]
    fn python_config_parses() {
        let text = r#"{"version": "2.0", "language": "de", "chat_path": "",
            "modes": {"email": {"provider": "anthropic", "model": "claude-haiku-4-5",
            "max_tokens": 2048, "reasoning_effort": "none"}}}"#;
        let cfg: Config = serde_json::from_str(text).unwrap();
        assert_eq!(cfg.mode("email").model, "claude-haiku-4-5");
        assert_eq!(cfg.mode("translate").model, "gpt-6-luna");
    }

    #[test]
    fn email_texts_match_python_version() {
        assert!(EMAIL.starts_with("Output ONLY the corrected email text — no comments"));
        assert!(EMAIL.contains("nothing else.\n\nImprove only style"));
    }
}
