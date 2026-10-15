# Changelog

Alle nennenswerten Änderungen an KIterminal (kit) werden in dieser Datei dokumentiert.

## [0.12.0] — 2026-10-15

### Geändert
- **`-c` ist jetzt der Terminal-Assistent statt eines Coding-Assistenten.** Vor jeder Frage erkennt kit das OS samt Version, die Shell, aus der es gestartet wurde (PowerShell 7, Windows PowerShell, cmd, Git Bash, bash, zsh, fish, Nushell …), Admin- bzw. Root-Rechte und den aktuellen Ordner. Die Antwort beginnt mit dem Befehl für genau diese Shell, ohne Klickpfade durch die Oberfläche, und warnt bei riskanten Befehlen. Die Shell wird über die Elternprozesse bestimmt, nicht über `$SHELL`.
- Bei `-c` landet nur der Befehl in der Zwischenablage, also der erste Codeblock der Antwort.
- Neuer Modus `terminal` mit eigener Instruction-Datei `terminal.txt`. Default ist Codex/`gpt-6-luna`, auch wenn eine bestehende Config den Modus noch nicht kennt. Der alte Modus `codex` und eine vorhandene `codex.txt` werden nicht mehr verwendet, aber auch nicht gelöscht.
- Die Langform heisst jetzt `--command`, `--codex` funktioniert weiterhin.

## [0.11.0] — 2026-10-15

### Geändert
- **Neu in Rust geschrieben.** Ein Start dauert rund 15 ms statt rund 3 s bei der Python-Version mit PyInstaller. Das Binary ist 2,6 MB statt 23 MB gross und braucht keine Laufzeit.
- Alle Provider (Codex, Anthropic, DeepSeek) laufen direkt über HTTP mit Streaming, ohne SDK. Ein Proxy wird aus der Umgebung übernommen.
- Markdown wird beim Streamen formatiert: Überschriften, Listen, Zitate, Codeblöcke, **fett** und `code`. Ohne Terminal oder mit `NO_COLOR` kommt Rohtext.
- Mail und Übersetzung zeigen bis zur Antwort einen Spinner. Der Chat zeigt ihn bis zum ersten Zeichen.
- Ctrl+C bricht eine laufende Antwort ab und speichert den Chat. Zweimal Ctrl+C beendet sofort.
- `kit -m "Hinweis"` mit Piped-Input nimmt jetzt beide Teile, vorher ging der Piped-Text verloren.
- `kit -s` und `kit -e` öffnen `$EDITOR`, falls gesetzt.
- Config, Instruction-Dateien und Chat-Verläufe bleiben im selben Format, bestehende Dateien werden weiter genutzt. Der Config-Key `Chatpath` aus v1 wird jetzt auch gelesen.
- Release-Assets und Installer bleiben gleich, ein Update läuft mit demselben Befehl. `install.ps1` schiebt die alte Exe vorher als `kit.exe.old` beiseite, so klappt das Update auch, wenn sie gesperrt ist.

### Entfernt
- Python-Code und pipx-Installation. Aus dem Quellcode: `cargo install --git https://github.com/jli-software/kit`

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
