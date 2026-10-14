"""
Configuration Management für KIterminal.

Config-Datei: ~/.config/KIterminal/config.json
Instruction-Files: ~/.config/KIterminal/instructions/*.txt

Neues Format (v2.0) — pro Modus: provider + model + optionale Einstellungen.
"""

import json
import os
import sys
from pathlib import Path

home_dir = Path.home()
config_dir = home_dir / ".config" / "KIterminal"
instructions_dir = config_dir / "instructions"

config_dir.mkdir(parents=True, exist_ok=True)
instructions_dir.mkdir(parents=True, exist_ok=True)

config_file = config_dir / "config.json"


def load_instruction(filename: str, language: str = "en") -> str:
    """
    Lädt eine Instruction-Datei mit automatischem Sprach-Präfix.

    Args:
        filename: Name der Instruction-Datei (z.B. 'normalchat.txt')
        language: Antwort-Sprachcode ('en', 'de', 'chde', 'fr', …)

    Returns:
        Inhalt der Instruction-Datei mit Sprach-Präfix, oder Leerstring.
    """
    instruction_file = instructions_dir / filename
    if not instruction_file.exists():
        return ""

    base_instruction = instruction_file.read_text(encoding="utf-8")

    language_prefixes = {
        "en": "IMPORTANT: Always respond in English.\n\n",
        "de": "IMPORTANT: Always respond in German.\n\n",
        "chde": "IMPORTANT: Always respond in Swiss German.\n\n",
        "fr": "IMPORTANT: Always respond in French.\n\n",
        "es": "IMPORTANT: Always respond in Spanish.\n\n",
        "it": "IMPORTANT: Always respond in Italian.\n\n",
        "pt": "IMPORTANT: Always respond in Portuguese.\n\n",
        "nl": "IMPORTANT: Always respond in Dutch.\n\n",
    }

    prefix = language_prefixes.get(language.lower(), "")
    return prefix + base_instruction


def save_default_instructions() -> None:
    """Erstellt Default-Instruction-Dateien, falls sie nicht existieren."""
    defaults = {
        "normalchat.txt": (
            "Always respond in Markdown format.\n\n"
            "## Completeness & Persistence\n"
            "- Answer the question completely END-TO-END before stopping.\n"
            "- No unnecessary follow-up questions – make reasonable assumptions and document them at the end under 'Assumptions: …'.\n"
            "- When uncertain: mark briefly, but don't block.\n"
            "- Bias for action: If unclear → reasonable default assumption + justification.\n\n"
            "## Output Length & Structure\n"
            "- **Simple questions:** 2-4 sentences, no headings.\n"
            "- **Medium complexity:** 4-6 bullet points OR 6-8 sentences. Maximum 1 heading.\n"
            "- **Complex/multi-part questions:** Structured with **headings** (1-3 words, bold), bullet points allowed.\n"
            "- Bullet points only for steps/options/lists – not for everything.\n"
            "- No nested lists. No ANSI codes.\n\n"
            "## Tone & Style\n"
            "- **Efficiency shows respect:** Directly to the solution, no filler words.\n"
            "- Avoid: 'Sure!', 'Of course!', 'Got it!', 'Thanks for asking!' – start immediately with the answer.\n"
            "- Friendly but concise. No platitudes. Active voice.\n"
            "- Code/commands/paths in `backticks`. Never combine backticks with **.\n\n"
            "## Structure Guidelines\n"
            "- Headings: Optional, only when complexity justifies it. **Bold**, 1-3 words.\n"
            "- Bullet points: Use `-`. Combine related points. One line when possible.\n"
            "- Order: General → Specific → Supporting.\n"
            "- Code samples in fenced code blocks with language hint.\n\n"
            "## Conclusion\n"
            "- End with concrete result or next step (when appropriate).\n"
            "- No explicit confirmation questions like 'Does that work?' or 'Is that enough?'.\n"
        ),
        "codex.txt": (
            "You are Codex, a coding assistant. You are running as a coding agent in the KIterminal CLI on a user's computer.\n\n"
            "Always respond in Markdown format.\n\n"
            "## System Context\n"
            "- Operating System: {currentOS}\n"
            "- Platform: {platforminfo}\n\n"
            "## General\n"
            "- The arguments to `shell` will be passed to execvp(). Most terminal commands should be prefixed with [\"bash\", \"-lc\"].\n"
            "- Always set the `workdir` param when using the shell function. Do not use `cd` unless absolutely necessary.\n"
            "- When searching for text or files, prefer using `rg` or `rg --files` respectively because `rg` is much faster than alternatives like `grep`. (If the `rg` command is not found, then use alternatives.)\n\n"
            "## Editing constraints\n"
            "- Default to ASCII when editing or creating files. Only introduce non-ASCII or other Unicode characters when there is a clear justification and the file already uses them.\n"
            "- Add succinct code comments that explain what is going on if code is not self-explanatory.\n"
            "- You may be in a dirty git worktree.\n"
            "    * NEVER revert existing changes you did not make unless explicitly requested.\n"
            "    * If asked to make a commit or code edits and there are unrelated changes, don't revert those changes.\n"
            "    * If the changes are in files you've touched recently, read carefully and understand how you can work with the changes rather than reverting them.\n"
            "    * If the changes are in unrelated files, just ignore them and don't revert them.\n"
            "- While you are working, you might notice unexpected changes. If this happens, STOP IMMEDIATELY and ask the user how they would like to proceed.\n\n"
            "## Special user requests\n"
            "- If the user makes a simple request (such as asking for the time) which you can fulfill by running a terminal command (such as `date`), you should do so.\n"
            "- If the user asks for a \"review\", default to a code review mindset: prioritise identifying bugs, risks, behavioural regressions, and missing tests.\n\n"
            "## Presenting your work\n"
            "- Default: be very concise; friendly coding teammate tone.\n"
            "- Ask only when needed; suggest ideas; mirror the user's style.\n"
            "- For substantial work, summarize clearly.\n"
            "- Skip heavy formatting for simple confirmations.\n"
            "- Don't dump large files you've written; reference paths only.\n"
            "- Offer logical next steps (tests, commits, build) briefly.\n"
            "- For code changes: lead with a quick explanation, then more details on where and why.\n"
            "- When suggesting multiple options, use numeric lists so the user can quickly respond.\n\n"
            "### Final answer structure\n"
            "- Plain text; CLI handles styling. Use structure only when it helps scanability.\n"
            "- Headers: optional; short Title Case (1-3 words) wrapped in **…**.\n"
            "- Bullets: use - ; merge related points; keep to one line when possible.\n"
            "- Monospace: backticks for commands/paths/env vars/code ids.\n"
            "- Code samples in fenced code blocks; add a language hint whenever obvious.\n"
            "- Structure: group related bullets; order sections general → specific → supporting.\n"
            "- Tone: collaborative, concise, factual; present tense, active voice.\n"
            "- No nested bullets/hierarchies; no ANSI codes.\n"
            "- File References: use inline code to make file paths clickable; include line numbers."
        ),
        "email.txt": (
            "Output ONLY the corrected email text — no comments, no explanations, "
            "no correction lists, no headings, no extra formatting. Just the improved text, nothing else.\n\n"
            "Improve only style, spelling, punctuation, and clarity. "
            "Keep the original voice and tone. Do not add or remove content."
        ),
        "email_advanced.txt": (
            "Output ONLY the improved email text — no comments, no explanations, "
            "no correction lists, no headings, no extra formatting. Just the improved text, nothing else.\n\n"
            "Improve style, spelling, punctuation, clarity, and content. "
            "Strengthen weak phrasing, fix logical flow, remove redundancies, and sharpen the message. "
            "Keep the original intent, voice, and all facts. Do not add or remove relevant content."
        ),
        "email_pro.txt": (
            "Output ONLY the rewritten email text — no comments, no explanations, "
            "no headings, no extra formatting. Just the improved text, nothing else.\n\n"
            "Rewrite the email in a professional business tone. Use formal but clear language, "
            "logical structure, and concise phrasing. Add a clear call to action where appropriate. "
            "Keep the original intent and all facts. Eliminate informal phrasing, typos, and weak "
            "formulations. The result should read as polished business communication."
        ),
        "translate.txt": (
            "Always respond in Markdown format. "
            "Translate the text between German and English (auto-detect source language). "
            "Provide only the translated text, without explanations or additional content."
        ),
    }

    for filename, content in defaults.items():
        filepath = instructions_dir / filename
        if not filepath.exists():
            filepath.write_text(content, encoding="utf-8")


# ── Default-Konfiguration (v2.0) ──────────────────────────────────────────

config_default = {
    "version": "2.0",
    "language": "de",
    # Pro Modus: provider (codex|anthropic|deepseek), model, max_tokens, reasoning_effort
    # Default: ChatGPT-Abo via Codex-Login, kleinstes Modell
    "modes": {
        "normalchat": {
            "provider": "codex",
            "model": "gpt-6-luna",
            "max_tokens": 4096,
            "reasoning_effort": "low",
        },
        "codex": {
            "provider": "codex",
            "model": "gpt-6-luna",
            "max_tokens": 4096,
            "reasoning_effort": "medium",
        },
        "email": {
            "provider": "codex",
            "model": "gpt-6-luna",
            "max_tokens": 2048,
            "reasoning_effort": "low",
        },
        "email_advanced": {
            "provider": "codex",
            "model": "gpt-6-luna",
            "max_tokens": 4096,
            "reasoning_effort": "low",
        },
        "email_pro": {
            "provider": "codex",
            "model": "gpt-6-luna",
            "max_tokens": 4096,
            "reasoning_effort": "low",
        },
        "translate": {
            "provider": "codex",
            "model": "gpt-6-luna",
            "max_tokens": 2048,
            "reasoning_effort": "low",
        },
    },
    "chat_path": "",  # Leer = Default-Pfad ~/.config/KIterminal/KITchats/
}


def _migrate_v1_to_v2(old_config: dict) -> dict:
    """
    Migriert Config v1.x → v2.0.

    v1 hatte flache Keys wie "normalchat": "claude-sonnet-4-6",
    v2 hat "modes": {"normalchat": {"provider": "...", "model": "..."}}.
    """
    if "modes" in old_config and old_config.get("version") == "2.0":
        return old_config  # Bereits v2

    # Mode-Namen aus v1
    mode_map = {
        "normalchat": "normalchat",
        "codex": "codex",
        "email": "email",
        "email_advanced": "email_advanced",
        "email_pro": "email_pro",
        "translate": "translate",
        # v1-only modes – werden nicht migriert
        "Websearch": None,
        "ITSecurty_Websearch": None,
    }

    modes = {}
    for old_key, new_key in mode_map.items():
        if new_key is None:
            continue
        model = old_config.get(old_key, config_default["modes"][new_key]["model"])
        modes[new_key] = {
            # v1 war immer Anthropic; fehlende Modi bekommen den v2-Default
            "provider": "anthropic" if old_key in old_config else config_default["modes"][new_key]["provider"],
            "model": model,
            "max_tokens": config_default["modes"][new_key]["max_tokens"],
            "reasoning_effort": config_default["modes"][new_key]["reasoning_effort"],
        }

    return {
        "version": "2.0",
        "language": old_config.get("language", "de"),
        "modes": modes,
        "chat_path": old_config.get("Chatpath", ""),
    }


# ── Initialisierung ───────────────────────────────────────────────────────

save_default_instructions()

if not config_file.exists():
    with open(config_file, "w") as f:
        json.dump(config_default, f, indent=4)


def loadconfig() -> dict:
    """
    Lädt die Konfiguration aus config.json und Instructions aus Text-Dateien.
    Migriert automatisch von v1 → v2.
    """
    with open(config_file, "r") as f:
        config = json.load(f)

    # Migration prüfen
    if config.get("version") != "2.0":
        config = _migrate_v1_to_v2(config)
        # Gespeicherte Config aktualisieren
        with open(config_file, "w") as f:
            json.dump(config, f, indent=4)

    language = config.get("language", "en")

    # Instructions aus Dateien laden
    if "instructions" not in config:
        config["instructions"] = {}

    config["instructions"]["normalchat"] = load_instruction("normalchat.txt", language)
    config["instructions"]["codex"] = load_instruction("codex.txt", language)
    config["instructions"]["email"] = load_instruction("email.txt", language)
    config["instructions"]["email_advanced"] = load_instruction("email_advanced.txt", language)
    config["instructions"]["email_pro"] = load_instruction("email_pro.txt", language)
    config["instructions"]["translate"] = load_instruction("translate.txt", language)

    return config


def init_config() -> None:
    """Setzt die Konfiguration auf Werkseinstellungen zurück."""
    with open(config_file, "w") as f:
        json.dump(config_default, f, indent=4)
    save_default_instructions()


def get_mode_config(config: dict, mode: str) -> dict:
    """
    Gibt die Konfiguration für einen bestimmten Modus zurück.

    Returns:
        dict mit keys: provider, model, max_tokens, reasoning_effort
    """
    return config["modes"].get(mode, {})
