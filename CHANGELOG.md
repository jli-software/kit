# Changelog

Alle nennenswerten Änderungen an KIterminal (kit) werden in dieser Datei dokumentiert.

## [0.10.0] — 2026-10-14

### Hinzugefügt
- **Codex-Provider (`"provider": "codex"`):** nutzt das ChatGPT-Abo statt eines API-Keys. kit verwendet den Login von Codex CLI (`~/.codex/auth.json`, `$CODEX_HOME`), erneuert abgelaufene Tokens selbst und schreibt sie zurück. Spricht die Responses-API auf `chatgpt.com/backend-api/codex` per Streaming an.
- **Online-Installer:** `install.sh` (Linux/macOS, `curl … | sh`) und `install.ps1` (Windows, `irm … | iex`). Sie laden das passende Release-Binary, lösen die neuste Version über die GitHub-API auf (die `latest/download`-Links cached GitHub pro Datei), prüfen die SHA256-Summe und legen kit in den PATH.
- **CI:** Tests auf Linux, Windows und macOS bei jedem Push, `shellcheck` und ein PowerShell-Syntax-Check für die Installer.
- **Release-Workflow:** ein Tag `v*` baut Binaries für Linux x64/arm64, Windows x64 und macOS arm64 und erstellt das GitHub-Release mit `SHA256SUMS` und dem CHANGELOG-Abschnitt.
- Offline-Tests für den Codex-Provider (`tests/`)

### Geändert
- **Default-Config:** alle Modi auf `codex` / `gpt-6-luna` (kleines, günstiges Modell). Bestehende Configs bleiben unverändert, `kit -i` setzt sie zurück.
- `providers.stream_text()` als gemeinsame Schnittstelle für alle Provider. One-Shot-Modi (Mail, Übersetzung) streamen intern ebenfalls, die Ausgabe bleibt gleich.
- Entry-Point `kit.main:run` mit sauberem Ctrl+C auch bei pip/pipx-Installation
- Repo-Links auf `jli-software/kit`

### Entfernt
- `build.ps1`, `build.sh` (durch den Release-Workflow ersetzt), `claudeguide.txt`

## [0.9.1] — 2026-05-25

### Hinzugefügt
- **Streaming-Output:** Antworten erscheinen live im Terminal mit Rich `Live`+`Markdown`-Rendering
- **Startup-Banner:** Beim Start wird Provider, Modell und Version angezeigt
- **Dynamischer Prompt:** Prompt zeigt aktiven Provider + Modell (`deep/v4-flash 💬`)
- **`[project.scripts]`:** `kit = "kit:main"` → `pipx install .` macht `kit` global

### Gefixt
- **Markdown-Rendering:** Streaming nutzt jetzt `rich.live.Live` mit `rich.markdown.Markdown` — Tabellen, Code-Blocks, Listen werden live gerendert
- **Runtime-Deps:** `rich`, `pyperclip`, `prompt-toolkit` nach `[project].dependencies` verschoben (waren fälschlich in `[dependency-groups].dev` → `pipx` hat sie nicht installiert)
- **Paket-Struktur:** Flache `.py`-Dateien → `kit/`-Paket mit `__init__.py` (pipx braucht Packages, nicht lose Module)
- **Python-Version:** `requires-python` auf `<4.0` gelockert (CachyOS hat 3.14)

### Lessons Learned
- **`pipx` installiert nur `[project].dependencies`** — Runtime-Deps gehören NICHT in `[dependency-groups].dev`
- **`pipx` braucht Python-Packages** (Verzeichnis mit `__init__.py`), keine losen `.py`-Module
- **`[project.scripts]` Entry-Point** muss auf `paket:funktion` zeigen, nicht auf lockere Datei
- **Streaming + Markdown** kombinierbar via `rich.live.Live(Markdown(text))` — kein Trade-off nötig

### Geändert
- Chat-Modi (normalchat, codex) streamen jetzt live; Mail/Translate bleiben bei One-Shot ohne Stream
- Verbesserte Fehlerbehandlung: `_call_api()` fängt `ValueError` von `get_client()` sauber ab

## [0.9.0] — 2026-05-25

### Hinzugefügt
- **Multi-Provider-Support:** Anthropic Claude + DeepSeek V4 (Flash & Pro)
- `providers.py` — Provider-Abstraktion, DeepSeek via Anthropic-kompatiblen Endpoint
- API-Keys aus Umgebungsvariablen: `ANTHROPIC_API_KEY`, `DEEPSEEK_API_KEY`
- Config v2.0 mit pro-Modus Provider/Modell/max_tokens/reasoning_effort
- Automatische Migration von v1.x Configs

### Entfernt
- Websearch-Modus (`-w`) — durch vollwertige Agents (Claude Code, Codex CLI) abgedeckt
- IT-Security-News-Modus (`-it`) — gleicher Grund

### Geändert
- CLI-Texte auf Deutsch
- Code-Struktur bereinigt, `_call_api()` als zentrale API-Schnittstelle
- README komplett neu, auf Deutsch, mit Multi-Provider-Dokumentation

## [0.8.2] und früher

Siehe Git-History für Details zu älteren Versionen.
