//! Chat-Verläufe: ~/.config/KIterminal/KITchats/{mode}_{timestamp}.json
//! (gleiches Format wie die Python-Version, alte Chats lassen sich fortsetzen).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{self, Config};
use crate::providers::Message;
use crate::render::{CYAN, DIM, RED, YELLOW, paint};

#[derive(Serialize, Deserialize)]
struct ChatFile {
    mode: String,
    timestamp: String,
    history: Vec<Message>,
}

pub fn chat_dir(cfg: &Config) -> PathBuf {
    let custom = cfg.chat_path.trim();
    let dir = if custom.is_empty() {
        config::config_dir().join("KITchats")
    } else {
        PathBuf::from(custom).join("KITchats")
    };
    let _ = fs::create_dir_all(&dir);
    dir
}

fn now_stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

/// Speichert den Chat. Mit `existing` wird die Datei überschrieben
/// (Timestamp bleibt), sonst entsteht eine neue Datei. Gibt den Pfad zurück.
pub fn save(
    cfg: &Config,
    history: &[Message],
    mode: &str,
    existing: Option<&Path>,
) -> Option<PathBuf> {
    if history.is_empty() {
        return None;
    }
    let (path, timestamp) = match existing {
        Some(p) => {
            let ts = fs::read_to_string(p)
                .ok()
                .and_then(|t| serde_json::from_str::<ChatFile>(&t).ok())
                .map(|c| c.timestamp)
                .unwrap_or_else(now_stamp);
            (p.to_path_buf(), ts)
        }
        None => {
            let ts = now_stamp();
            (chat_dir(cfg).join(format!("{mode}_{ts}.json")), ts)
        }
    };
    let data = ChatFile {
        mode: mode.into(),
        timestamp,
        history: history.to_vec(),
    };
    let json = serde_json::to_string_pretty(&data).ok()?;
    fs::write(&path, json).ok()?;
    Some(path)
}

pub fn load(path: &Path) -> Result<Vec<Message>, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let chat: ChatFile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(chat.history)
}

struct Entry {
    path: PathBuf,
    timestamp: String,
    preview: String,
    count: usize,
}

fn list(cfg: &Config, mode: &str) -> Vec<Entry> {
    let Ok(dir) = fs::read_dir(chat_dir(cfg)) else {
        return Vec::new();
    };
    let prefix = format!("{mode}_");
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = dir
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(&prefix) && name.ends_with(".json")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0)); // neueste zuerst

    files
        .into_iter()
        .filter_map(|(_, path)| {
            let chat: ChatFile = serde_json::from_str(&fs::read_to_string(&path).ok()?).ok()?;
            let first = chat
                .history
                .iter()
                .find(|m| m.role == "user")
                .map(|m| m.content.as_str())
                .unwrap_or("");
            let clean = first.split_whitespace().collect::<Vec<_>>().join(" ");
            let preview = if clean.chars().count() > 60 {
                clean.chars().take(60).collect::<String>() + "..."
            } else {
                clean
            };
            Some(Entry {
                path,
                timestamp: chat.timestamp,
                preview,
                count: chat.history.len(),
            })
        })
        .collect()
}

/// Auswahlmenü für gespeicherte Chats; `None` = neuen Chat starten.
pub fn select(cfg: &Config, mode: &str) -> Option<PathBuf> {
    let chats = list(cfg, mode);
    if chats.is_empty() {
        println!(
            "{}",
            paint(
                YELLOW,
                &format!("Keine gespeicherten {mode}-Chats gefunden.")
            )
        );
        return None;
    }
    println!(
        "\n{}\n",
        paint(CYAN, &format!("Gespeicherte {mode}-Chats:"))
    );
    for (i, c) in chats.iter().enumerate() {
        // 2025-11-24_14-06-38 → 2025:11:24 14:06:38
        let ts = match c.timestamp.split_once('_') {
            Some((d, t)) => format!("{} {}", d.replace('-', ":"), t.replace('-', ":")),
            None => c.timestamp.clone(),
        };
        println!(
            "{}. {ts} - {} {}",
            paint(CYAN, &(i + 1).to_string()),
            c.preview,
            paint(DIM, &format!("({} Nachrichten)", c.count))
        );
    }
    println!("{}. Neuen Chat starten\n", paint(CYAN, "0"));

    loop {
        print!("{} (0): ", paint(CYAN, "Wähle einen Chat"));
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
            return None;
        }
        let choice = line.trim();
        match if choice.is_empty() {
            Ok(0)
        } else {
            choice.parse::<usize>()
        } {
            Ok(0) => return None,
            Ok(n) if n <= chats.len() => return Some(chats[n - 1].path.clone()),
            Ok(_) => println!(
                "{}",
                paint(RED, "Ungültige Auswahl, bitte versuche es erneut.")
            ),
            Err(_) => println!("{}", paint(RED, "Bitte gib eine Zahl ein.")),
        }
    }
}
