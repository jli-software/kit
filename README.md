# Kit — KIterminal

**Ein schlankes Terminal-KI-Tool für schnelle Aufgaben.** In Rust geschrieben: ein Binary von rund 2,5 MB, das in wenigen Millisekunden startet.

Multi-Provider-Support: **OpenAI Codex über dein ChatGPT-Abo** (kein API-Key nötig) + **Anthropic Claude** + **DeepSeek V4**. Entwickelt als Ergänzung zu vollwertigen Coding-Agents (Claude Code, Codex CLI) — für die kleinen, schnellen Dinge zwischendurch.

## Features

- 🤖 **Multi-Provider** — OpenAI Codex (ChatGPT-Login), Anthropic Claude, DeepSeek V4; pro Modus konfigurierbar
- 💬 **Chat-Modi**
  - Normaler Chat (default)
  - Codex — Programmier-Assistent
- ✉️ **E-Mail** — Korrektur (`-m`), Verbesserung (`-ma`), Professioneller Rewrite (`-mp`)
- 🌐 **Übersetzung** — DE↔EN aus Clipboard oder direkt (`-t`)
- 📋 **Clipboard-Integration** — Input aus Clipboard lesen, Antwort automatisch kopieren
- 🔄 **Pipe-Support** — `cat file.txt | kit "Zusammenfassen"` + interaktiver Follow-up
- 💾 **Chat-History** — Automatisches Speichern, Fortsetzen mit `-r`

## Installation

### Online (empfohlen)

Lädt das passende Binary der neusten Version, prüft die SHA256-Summe und legt es in den PATH. Weitere Abhängigkeiten braucht es nicht. Zum Aktualisieren den gleichen Befehl nochmals ausführen.

**Linux / macOS** (x64 und arm64, Ziel `~/.local/bin`):

```bash
curl -fsSL https://raw.githubusercontent.com/jli-software/kit/master/install.sh | sh
```

**Windows** (PowerShell, Ziel `%LOCALAPPDATA%\Programs\kit`, Benutzer-PATH wird ergänzt):

```powershell
irm https://raw.githubusercontent.com/jli-software/kit/master/install.ps1 | iex
```

Bestimmte Version: `KIT_VERSION=v0.11.0` bzw. `$env:KIT_VERSION = 'v0.11.0'`. Anderer Zielordner: `KIT_INSTALL_DIR`.

### Aus dem Quellcode (Rust)

```bash
cargo install --git https://github.com/jli-software/kit
```

Linux braucht für die Zwischenablage `wl-clipboard` (Wayland) oder `xclip`/`xsel` (X11).

### Provider einrichten

**OpenAI Codex (Default)** — nutzt dein ChatGPT-Abo statt API-Guthaben. kit verwendet den Login von [Codex CLI](https://github.com/openai/codex), einmal anmelden genügt:

```bash
codex login        # "Sign in with ChatGPT"
```

kit liest die Tokens aus `~/.codex/auth.json` (bzw. `$CODEX_HOME`) und erneuert sie selbst, wenn sie ablaufen. Anfragen zählen gegen das Nutzungskontingent deines ChatGPT-Abos. Die Schnittstelle (`chatgpt.com/backend-api/codex`) ist nicht offiziell dokumentiert und kann sich ändern.

**Anthropic / DeepSeek** — per API-Key (optional):

```bash
export ANTHROPIC_API_KEY='dein-anthropic-key'
export DEEPSEEK_API_KEY='dein-deepseek-key'
```

## Verwendung

### Normaler Chat

```bash
kit                     # Interaktiver Chat
kit -r                  # Vorherigen Chat fortsetzen
```

### Codex (Programmier-Assistent)

```bash
kit -c                  # Interaktiv
kit -c "Wie sortiere ich eine Liste in Python?"
kit -c -r               # Codex-Chat fortsetzen
```

### E-Mail-Korrektur

```bash
kit -m                  # Aus Clipboard lesen + korrigieren
kit -m "Hallo, ich wollte fragen ob..."
kit -ma                 # Korrektur + inhaltliche Verbesserung
kit -mp                 # Professioneller Business-Rewrite
```

### Übersetzung

```bash
kit -t                  # Aus Clipboard
kit -t "Hello World"
```

### Pipe-Support

```bash
# Code Review
cat script.py | kit -c "Review diesen Code"

# Log-Analyse
tail -100 app.log | kit "Fasse die Fehler zusammen"

# E-Mail aus Datei
cat email.txt | kit -m

# Übersetzung aus Datei
cat dokument.txt | kit -t
```

Nach der Pipe-Verarbeitung wechselt kit automatisch in den interaktiven Modus für Folgefragen.

## Konfiguration

### Provider & Modell pro Modus

Bearbeite `~/.config/KIterminal/config.json`:

```json
{
  "version": "2.0",
  "language": "de",
  "modes": {
    "normalchat": {
      "provider": "codex",
      "model": "gpt-6-luna",
      "max_tokens": 4096,
      "reasoning_effort": "low"
    },
    "email": {
      "provider": "anthropic",
      "model": "claude-haiku-4-5",
      "max_tokens": 2048,
      "reasoning_effort": "none"
    }
  }
}
```

**Provider-Optionen:** `codex`, `anthropic`, `deepseek`  
**Codex-Modelle:** was dein ChatGPT-Abo in Codex anbietet, z. B. `gpt-6-luna` (klein, günstig — Default), `gpt-6-sol`, `gpt-6-astra`  
**DeepSeek-Modelle:** `deepseek-v4-flash` (schnell), `deepseek-v4-pro` (stark)  
**Reasoning Effort:** `none`, `low`, `medium`, `high` (bei Codex wird `none` zu `low`)  
`max_tokens` gilt nur für Anthropic/DeepSeek.

Fehlt ein Modus in der Datei, gilt der Default (Codex mit `gpt-6-luna`). Bestehende Configs werden nicht überschrieben. Auf Codex umstellen geht mit `kit -i` (Werkseinstellungen) oder von Hand.

### Config-Editor

```bash
kit --setup             # Config mit Editor öffnen ($EDITOR, sonst System-Standard)
kit -e codex            # Codex-Instructions bearbeiten
kit -e normalchat       # Chat-Instructions bearbeiten
kit -i                  # Auf Werkseinstellungen zurücksetzen
```

### Sprache

Setze `"language": "de"` in der Config für deutsche Antworten.  
Unterstützt: `en`, `de`, `chde`, `fr`, `es`, `it`, `pt`, `nl`

## Architektur

```
src/main.rs        — CLI (clap), Modi, Chat-Loop (rustyline)
src/config.rs      — Config v2.0 + Instruction-Dateien
src/providers.rs   — Codex (Responses-API), Anthropic/DeepSeek (Messages-API), alles gestreamt
src/codex_auth.rs  — ChatGPT-OAuth: Tokens aus Codex CLI lesen und erneuern
src/http.rs        — HTTP-Agent (ureq, rustls, Proxy aus der Umgebung) + SSE-Leser
src/render.rs      — Markdown-Formatierung beim Streamen, Spinner
src/history.rs     — Chat-Verläufe
src/platform.rs    — Zwischenablage, Editor, Terminal nach Piped-Input
```

Alle Provider laufen direkt über HTTP, ohne SDK. Config, Instruction-Dateien und Chat-Verläufe haben dasselbe Format wie die frühere Python-Version (≤ 0.10), bestehende Dateien werden weiter genutzt.

## Entwicklung & Releases

```bash
cargo test
cargo run -- -t "Hallo Welt"
```

- **CI** (`.github/workflows/ci.yml`): `cargo fmt --check`, `clippy -D warnings` und Tests auf Linux, Windows und macOS bei jedem Push, dazu `shellcheck` für `install.sh` und ein Syntax-Check für `install.ps1`.
- **Release** (`.github/workflows/release.yml`): Version in `Cargo.toml` hochzählen, CHANGELOG-Abschnitt schreiben, dann Tag pushen:

  ```bash
  git tag v0.11.0 && git push origin v0.11.0
  ```

  Der Workflow baut auf den jeweiligen Runnern Binaries für Linux x64/arm64, Windows x64 und macOS arm64, prüft dass sie starten, und erstellt das GitHub-Release mit `SHA256SUMS` und dem CHANGELOG-Abschnitt als Text.

## Plattform-Support

- **Linux** — vollständig
- **macOS** — vollständig
- **Windows** — vollständig
- **WSL (Windows)** — vollständig
- **Termux (Android)** — Clipboard-Integration angepasst

## Tipps

1. **Pipe + Follow-up** — `cat file | kit -c "Erklären"` und dann direkt Folgefragen stellen
2. **Chat fortsetzen** — `-r` lädt den letzten Chat mit allen Kontext
3. **Clipboard-Workflow** — Text kopieren → `kit -m` oder `kit -t` → Antwort ist schon im Clipboard
4. **Reasoning steuern** — `reasoning_effort: "high"` für komplexe Fragen, `"none"` für schnelle Antworten
5. **Provider pro Aufgabe** — Codex `gpt-6-luna` für Mail/Übersetzung, grössere Modelle nur wo nötig

## License

MIT — [JLI-Software](https://github.com/jli-software)
