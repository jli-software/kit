<div align="center">
  <img src="kit-icon.svg" width="120" alt="kit logo" />
  <h1>kit</h1>
  <p>A tiny AI sidekick for your terminal — fix an email, translate a text,<br/>ask a quick question. Starts in milliseconds, runs on your ChatGPT plan.</p>

  ![Version](https://img.shields.io/badge/version-0.11.0-ff3b30?style=flat-square)
  ![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-89b4fa?style=flat-square)
  ![License](https://img.shields.io/badge/license-MIT-a6e3a1?style=flat-square)
  ![Built with Rust](https://img.shields.io/badge/built%20with-Rust-f5a97f?style=flat-square)
</div>

---

**kit** (short for *KI-Terminal*, and yes, a nod to K.I.T.T.) handles the small things in between: proofreading, translations and quick questions, without opening a browser or a full coding agent. Copy a text, run `kit -m`, and the corrected version is already back in your clipboard.

Ships as a single ~2.5 MB binary, with no runtime and no dependencies.

---

## Install

**Linux / macOS**

```bash
curl -fsSL https://raw.githubusercontent.com/jli-software/kit/master/install.sh | sh
```

**Windows** (PowerShell)

```powershell
irm https://raw.githubusercontent.com/jli-software/kit/master/install.ps1 | iex
```

Run the same command again to update. The installers verify the SHA256 checksum and add kit to your `PATH`. Pin a version with `KIT_VERSION=v0.11.0`; change the target folder with `KIT_INSTALL_DIR`.

From source: `cargo install --git https://github.com/jli-software/kit`

## Sign in

kit uses your **ChatGPT subscription** through the [Codex CLI](https://github.com/openai/codex) login, so no API key is needed:

```bash
codex login
```

kit reads the tokens from `~/.codex/auth.json` and refreshes them on its own. Requests count towards your ChatGPT usage limits. The Codex backend is not an official public API and may change.

Prefer API keys? Set `ANTHROPIC_API_KEY` or `DEEPSEEK_API_KEY` and pick that provider in the config.

## Usage

| Command | What it does |
|---|---|
| `kit` | Interactive chat |
| `kit "question"` | Chat, starting with a question |
| `kit -m` | Proofread an email (from clipboard, or `kit -m "text"`) |
| `kit -ma` | Proofread and improve wording |
| `kit -mp` | Rewrite in a professional business tone |
| `kit -t` | Translate German ↔ English |
| `kit -c` | Coding assistant (interactive or `kit -c "question"`) |
| `kit -r` | Resume a previous chat (`kit -r -c` for coding chats) |

Results are copied to the clipboard automatically. You can also pipe text in:

```bash
cat app.log | kit "Summarize the errors"
cat email.txt | kit -m
```

After a piped question, kit stays open for follow-ups. Press Ctrl+C to stop an answer; the chat is saved.

## Configuration

`kit -s` opens `~/.config/KIterminal/config.json`. You can set a provider and model per mode:

```json
"email": {
  "provider": "codex",
  "model": "gpt-6-luna",
  "max_tokens": 2048,
  "reasoning_effort": "low"
}
```

- **Providers:** `codex` (default), `anthropic`, `deepseek`
- **Models:** for `codex`, any model your ChatGPT plan offers in Codex. The default is `gpt-6-luna`, the small, cheap one.
- **Reasoning effort:** `none`, `low`, `medium`, `high`
- **Answer language:** `"language": "de"`. Supported: `en`, `de`, `chde`, `fr`, `es`, `it`, `pt`, `nl`.

`kit -e <mode>` edits the system prompt of a mode. `kit -i` resets everything to defaults.

## Development

```bash
cargo test
cargo run -- -t "Hallo Welt"
```

CI runs `fmt`, `clippy` and the tests on Linux, Windows and macOS. To release, bump the version in `Cargo.toml`, add a `CHANGELOG.md` entry and push a tag (`git tag v0.11.0 && git push origin v0.11.0`). GitHub Actions then builds the binaries and publishes the release.

## License

MIT — [JLI-Software](https://github.com/jli-software)
