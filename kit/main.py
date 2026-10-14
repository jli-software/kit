#!/usr/bin/env python3
"""
KIterminal — Terminal-KI-Tool für schnelle Aufgaben.

Multi-Provider: Anthropic + DeepSeek + OpenAI Codex (ChatGPT-Login)
Modi: chat, codex, mail (-m/-ma/-mp), translate (-t)
Features: Streaming, Pipe-Support, Clipboard, Chat-History
"""

import anthropic
import sys
import argparse
import platform
import subprocess
import os
from rich.console import Console
from rich.markdown import Markdown
from rich.live import Live
from rich.prompt import Prompt
from prompt_toolkit import prompt as pt_prompt
from prompt_toolkit.formatted_text import HTML
from prompt_toolkit.key_binding import KeyBindings
import pyperclip

from kit.config import loadconfig, init_config, get_mode_config
from kit.chat_history import save_chat, load_chat, show_chat_selection_menu
from kit import __version__, chat_history as ch
from kit.codex_auth import auth_file
from kit.providers import stream_text, get_provider_name, API_ERRORS

AUTHOR = "JLI-Software"
VERSION = __version__
LICENSE = "MIT"
ANTHROPIC_VERSION = anthropic.__version__
DESCRIPTION = "KIterminal — Schlankes KI-CLI-Tool. Multi-Provider: Anthropic + DeepSeek + OpenAI Codex."

# ── CLI-Parser ────────────────────────────────────────────────────────────

parser = argparse.ArgumentParser(
    prog="kit",
    description=DESCRIPTION,
    epilog="""
Beispiele:
  kit                     Chat (default)
  kit -c                  Codex Programmier-Assistent
  kit -c "Frage"          Codex One-Shot
  kit -m                  E-Mail-Korrektur (aus Clipboard)
  kit -m "Text"           E-Mail-Korrektur (direkt)
  kit -ma "Text"          E-Mail-Korrektur + Verbesserung
  kit -mp "Text"          Professioneller E-Mail-Rewrite
  kit -t "Hello World"    Uebersetzung DE<->EN
  kit -r                  Chat fortsetzen
  kit -r -c               Codex-Chat fortsetzen
  kit --setup             Config bearbeiten
  kit -e codex            Codex-Instructions bearbeiten
  kit --version           Version anzeigen

Piped Input:
  cat file.txt | kit "Zusammenfassen"
  cat script.py | kit -c "Erklaeren"
  cat email.txt | kit -m "Korrigieren"
  cat doc.txt | kit -t "Uebersetzen"

Konfiguration:
  kit -e normalchat       Chat-Instructions bearbeiten
  kit -e codex            Codex-Instructions bearbeiten
  kit -e email            Mail-Instructions bearbeiten
  kit -e translate        Uebersetzungs-Instructions bearbeiten

  Instruction-Dateien: ~/.config/KIterminal/instructions/
  Config-Datei:        ~/.config/KIterminal/config.json

Provider-Login:
  codex                   ChatGPT-Abo, einmal 'codex login' (Codex CLI)
  ANTHROPIC_API_KEY       Anthropic API-Key
  DEEPSEEK_API_KEY        DeepSeek API-Key

Mehr Infos: https://github.com/jli-software/kit
    """,
    formatter_class=argparse.RawDescriptionHelpFormatter,
)

# ── Modi ──────────────────────────────────────────────────────────────────

ai_modes = parser.add_argument_group("KI-Modi")
ai_modes.add_argument(
    "-c", "--codex",
    nargs="?", const=True, metavar="TEXT",
    help="Codex Programmier-Assistent (interaktiv oder One-Shot)",
)

# ── Textverarbeitung ──────────────────────────────────────────────────────

text_proc = parser.add_argument_group("Textverarbeitung")
text_proc.add_argument(
    "-t", "--translate",
    nargs="?", const=True, metavar="TEXT",
    help="Uebersetzung DE<->EN (Clipboard oder Text)",
)
text_proc.add_argument(
    "-m", "--mail",
    nargs="?", const=True, metavar="TEXT",
    help="E-Mail-Korrektur (Clipboard oder Text)",
)
text_proc.add_argument(
    "-ma", "--mail-advanced",
    nargs="?", const=True, metavar="TEXT",
    help="E-Mail-Korrektur + inhaltliche Verbesserung",
)
text_proc.add_argument(
    "-mp", "--mail-pro",
    nargs="?", const=True, metavar="TEXT",
    help="Professioneller E-Mail-Rewrite (Business-Ton)",
)

# ── Chat-Management ───────────────────────────────────────────────────────

chat_mgmt = parser.add_argument_group("Chat-Management")
chat_mgmt.add_argument(
    "-r", "--resume",
    action="store_true",
    help="Vorherigen Chat fortsetzen",
)

# ── Konfiguration ─────────────────────────────────────────────────────────

config_grp = parser.add_argument_group("Konfiguration")
config_grp.add_argument(
    "-s", "--setup", action="store_true",
    help="Config-Datei mit Editor oeffnen",
)
config_grp.add_argument(
    "-i", "--init", action="store_true",
    help="Config auf Werkseinstellungen zuruecksetzen",
)
config_grp.add_argument(
    "-e", "--edit-instructions", metavar="MODE",
    help="Instructions fuer einen Modus bearbeiten "
         "(normalchat, codex, email, email-advanced, email-pro, translate)",
)

# ── Info ──────────────────────────────────────────────────────────────────

info_grp = parser.add_argument_group("Information")
info_grp.add_argument(
    "-v", "--version", action="store_true",
    help="Version & Info anzeigen",
)

# Positionales Argument (fuer Piped-Input + Frage)
parser.add_argument(
    "question", nargs="?", default=None,
    help="Frage (erforderlich bei Piped-Input)",
)

args = parser.parse_args()


# ── Umgebung ──────────────────────────────────────────────────────────────

console = Console()
currentOS = platform.system()
platforminfo = platform.platform()
config = loadconfig()

# Chat-History (in-memory, global)
chat_history: list[dict] = []


# ── Prompt-Toolkit Setup ──────────────────────────────────────────────────

kb = KeyBindings()

@kb.add("enter")
def _(event):
    event.current_buffer.validate_and_handle()


def get_user_input(label: str = ">") -> str:
    """Benutzereingabe mit prompt_toolkit."""
    try:
        return pt_prompt(
            HTML(f"<ansibrightcyan><b>{label} </b></ansibrightcyan>"),
            multiline=False,
            key_bindings=kb,
        ).strip()
    except KeyboardInterrupt:
        raise
    except EOFError:
        return "exit"


# ── Plattform-Helfer ─────────────────────────────────────────────────────

def is_termux() -> bool:
    return os.path.exists("/data/data/com.termux")


def is_wsl() -> bool:
    try:
        with open("/proc/version") as f:
            content = f.read().lower()
            return "microsoft" in content or "wsl" in content
    except Exception:
        return False


def read_stdin() -> str | None:
    """Liest Piped-Input von stdin, falls vorhanden."""
    if sys.stdin.isatty():
        return None
    return sys.stdin.read().strip()


def reopen_stdin_to_terminal() -> bool:
    """Oeffnet stdin neu zum Terminal (nach Piped-Input)."""
    try:
        if currentOS == "Windows" and not is_wsl():
            sys.stdin = open("CON", "r")
        else:
            sys.stdin = open("/dev/tty", "r")
        return True
    except (OSError, FileNotFoundError, PermissionError):
        return False


# ── Clipboard ─────────────────────────────────────────────────────────────

def copyline(text: str) -> None:
    """Text ins Clipboard kopieren (silent fail)."""
    try:
        if is_termux():
            subprocess.run(["termux-clipboard-set"], input=text.encode(), check=True)
        elif is_wsl():
            subprocess.run(["clip.exe"], input=text.encode(), check=True)
        else:
            pyperclip.copy(text)
    except Exception:
        pass


def pasteline() -> str:
    """Text aus Clipboard lesen."""
    try:
        if is_termux():
            result = subprocess.run(
                ["termux-clipboard-get"], capture_output=True, text=True, check=True
            )
            return result.stdout
        elif is_wsl():
            result = subprocess.run(
                ["powershell.exe", "-command", "Get-Clipboard"],
                capture_output=True, text=True, check=True,
            )
            return result.stdout.strip()
        else:
            return pyperclip.paste()
    except Exception as e:
        console.print(f"[red]✗ Clipboard-Fehler: {e}[/red]")
        sys.exit(1)


# ── API-Helfer ────────────────────────────────────────────────────────────

def handle_api_error(e: Exception) -> None:
    """Benutzerfreundliche Fehlermeldungen fuer API-Fehler."""
    if isinstance(e, anthropic.AuthenticationError):
        console.print("[red]✗ Ungueltiger API-Key![/red]")
        console.print("Pruefe ANTHROPIC_API_KEY / DEEPSEEK_API_KEY")
    elif isinstance(e, anthropic.APIConnectionError):
        console.print("[red]✗ Keine Verbindung zur API![/red]")
    elif isinstance(e, anthropic.RateLimitError):
        console.print("[red]✗ Rate-Limit erreicht – warte einen Moment[/red]")
    elif isinstance(e, anthropic.APIStatusError):
        console.print(f"[red]✗ API-Fehler {e.status_code}: {e}[/red]")
    elif isinstance(e, API_ERRORS):
        console.print(f"[red]✗ {e}[/red]")
    else:
        console.print(f"[red]✗ Unerwarteter Fehler: {e}[/red]")


def build_piped_message(question: str, stdin_content: str) -> str:
    """Kombiniert Frage mit Piped-Input."""
    return f"{question}\n\n```\n{stdin_content}\n```"


# ── Chat-Funktionen ───────────────────────────────────────────────────────

def _call_api(mode: str, messages: list[dict], system: str, stream: bool = True) -> str:
    """
    Fuehrt einen API-Call fuer den angegebenen Modus aus.
    Bei stream=True wird die Antwort live mit Markdown-Rendering ausgegeben.
    """
    mode_cfg = get_mode_config(config, mode)
    provider = mode_cfg.get("provider", "anthropic")
    model = mode_cfg.get("model", "claude-sonnet-4-6")

    try:
        chunks = stream_text(
            provider,
            model=model,
            system=system,
            messages=messages,
            max_tokens=mode_cfg.get("max_tokens", 4096),
            reasoning_effort=mode_cfg.get("reasoning_effort", "none"),
        )
    except ValueError as e:
        console.print(f"[red]✗ {e}[/red]")
        sys.exit(1)

    full_text: list[str] = []
    if stream:
        try:
            with Live(Markdown(""), console=console, refresh_per_second=10,
                      vertical_overflow="visible") as live:
                for text in chunks:
                    full_text.append(text)
                    live.update(Markdown("".join(full_text)))
        except KeyboardInterrupt:
            # Sauberer Abbruch – Live-Display wird automatisch geschlossen,
            # teilweise gestreamter Text wird trotzdem gespeichert
            if full_text:
                console.print()
            raise
    else:
        with console.status(f"[bold green]{get_provider_name(provider)}/{model} …[/bold green]"):
            full_text.extend(chunks)
    return "".join(full_text)


def normalchat(userfrage: str) -> None:
    """Normaler Chat-Modus (interaktiv)."""
    chat_history.append({"role": "user", "content": userfrage})
    try:
        response_text = _call_api(
            "normalchat",
            messages=chat_history,
            system=config["instructions"]["normalchat"],
        )
        chat_history.append({"role": "assistant", "content": response_text})
        copyline(response_text)
    except API_ERRORS as e:
        handle_api_error(e)
        chat_history.pop()


def codex(userfrage: str) -> None:
    """Codex Programmier-Assistent (interaktiv)."""
    instruction = config["instructions"]["codex"].format(
        currentOS=currentOS, platforminfo=platforminfo
    )
    chat_history.append({"role": "user", "content": userfrage})
    try:
        response_text = _call_api(
            "codex",
            messages=chat_history,
            system=instruction,
        )
        chat_history.append({"role": "assistant", "content": response_text})
        copyline(response_text)
    except API_ERRORS as e:
        handle_api_error(e)
        chat_history.pop()


def mail_correct(userfrage: str, mode: str = "email") -> None:
    """E-Mail-Korrektur (One-Shot)."""
    try:
        response_text = _call_api(
            mode,
            messages=[{"role": "user", "content": f"Proofread this email:\n\n{userfrage}"}],
            system=config["instructions"][mode],
            stream=False,
        )
        copyline(response_text)
        print(response_text)
    except API_ERRORS as e:
        handle_api_error(e)


def _run_mail(mode: str, args_flag, stdin_content: str | None, question: str | None) -> None:
    """Gemeinsame Routing-Logik fuer alle Mail-Modi (-m, -ma, -mp)."""
    if isinstance(args_flag, str):
        mail_correct(args_flag, mode=mode)
    elif stdin_content:
        content = build_piped_message(question, stdin_content) if question else stdin_content
        console.print("[green]Piped-Input wird verarbeitet[/green]")
        mail_correct(content, mode=mode)
    else:
        text = pasteline()
        console.print("[green]Aus Clipboard gelesen[/green]")
        mail_correct(text, mode=mode)


def translate(userfrage: str) -> None:
    """Uebersetzung DE<->EN (One-Shot)."""
    try:
        response_text = _call_api(
            "translate",
            messages=[{"role": "user", "content": f"Translate:\n\n{userfrage}"}],
            system=config["instructions"]["translate"],
            stream=False,
        )
        copyline(response_text)
        print(response_text)
    except API_ERRORS as e:
        handle_api_error(e)


# ── Interaktiver Chat-Loop ────────────────────────────────────────────────

def interactive_chat(turn_handler, mode: str = None, resume_chat: bool = False,
                     initial_message: str = None) -> None:
    """
    Interaktiver Chat mit Resume- und Auto-Save-Funktionalitaet.
    """
    global chat_history

    # ── Mode-Konfig einmal laden ───────────────────────────────────────
    mode_cfg = get_mode_config(config, mode) if mode else {}
    provider_name = mode_cfg.get("provider", "?")
    model_name = mode_cfg.get("model", "?")
    model_short = model_name.replace("claude-", "").replace("deepseek-", "")
    prompt_label = f"{provider_name[:4]}/{model_short} >" if mode else ">"

    if resume_chat and mode:
        selected = show_chat_selection_menu(mode)
        if selected:
            try:
                chat_history.clear()
                loaded = load_chat(selected)
                chat_history.extend(loaded)
                ch.current_chat_file = selected
                console.print(f"[green]Chat geladen ({len(chat_history)} Nachrichten)[/green]")
            except Exception as e:
                console.print(f"[red]✗ Fehler beim Laden: {e}[/red]")
                return
        else:
            ch.current_chat_file = None
            chat_history.clear()
    else:
        ch.current_chat_file = None
        chat_history.clear()

    # ── Startup-Banner ─────────────────────────────────────────────────
    if mode and not initial_message:
        provider_display = get_provider_name(provider_name)
        mode_names = {"normalchat": "Chat", "codex": "Codex"}
        mode_display = mode_names.get(mode, mode)
        console.print(
            f"[dim]kit v{VERSION}  ·  {provider_display}  ·  {model_name}  ·  {mode_display}[/dim]"
        )

    try:
        if initial_message:
            if not sys.stdin.isatty():
                console.print("[dim]Piped-Input wird verarbeitet ...[/dim]")
                turn_handler(initial_message)
                if not reopen_stdin_to_terminal():
                    console.print("[dim]Tipp: Mit -r kannst du diesen Chat spaeter fortsetzen[/dim]")
                    return
                console.print(f"[dim]Interaktiver Modus  —  {prompt_label}[/dim]")
            else:
                turn_handler(initial_message)

        while True:
            userfrage = get_user_input(prompt_label)
            if not userfrage:
                continue
            if userfrage.lower() == "exit":
                break
            turn_handler(userfrage)

    except KeyboardInterrupt:
        console.print()
        if mode and chat_history:
            save_chat(chat_history, mode, filepath=ch.current_chat_file)
        raise
    finally:
        if mode and chat_history:
            save_chat(chat_history, mode, filepath=ch.current_chat_file)
            console.print("[dim]Chat gespeichert[/dim]")
        ch.current_chat_file = None


# ── Config-Editor ─────────────────────────────────────────────────────────

def _open_with_editor(filepath: str, success_msg: str) -> None:
    """Oeffnet eine Datei mit dem System-Editor."""
    try:
        if currentOS == "Windows":
            os.startfile(filepath)
        elif currentOS == "Darwin":
            subprocess.run(["open", filepath])
        else:
            if subprocess.run(["which", "nvim"], capture_output=True).returncode == 0:
                subprocess.run(["nvim", filepath])
            else:
                subprocess.run(["xdg-open", filepath])
        console.print(f"[green]{success_msg}[/green]")
    except Exception as e:
        console.print(f"[red]✗ Fehler beim Oeffnen: {e}[/red]")


def show_setup_file() -> None:
    """Oeffnet die Config-Datei."""
    config_path = os.path.join(os.path.expanduser("~"), ".config", "KIterminal", "config.json")
    _open_with_editor(config_path, "Config-Datei geoeffnet")


def edit_instructions(mode: str) -> None:
    """Oeffnet die Instruction-Datei eines Modus."""
    instructions_dir = os.path.join(os.path.expanduser("~"), ".config", "KIterminal", "instructions")
    mode_files = {
        "normalchat": "normalchat.txt", "codex": "codex.txt",
        "email": "email.txt", "email-advanced": "email_advanced.txt",
        "email-pro": "email_pro.txt", "translate": "translate.txt",
    }
    if mode.lower() not in mode_files:
        console.print(f"[red]✗ Unbekannter Modus: {mode}[/red]")
        console.print(f"[yellow]Verfuegbar: {', '.join(sorted(mode_files.keys()))}[/yellow]")
        return
    instruction_file = os.path.join(instructions_dir, mode_files[mode.lower()])
    if not os.path.exists(instruction_file):
        console.print(f"[red]✗ Datei nicht gefunden: {instruction_file}[/red]")
        return
    _open_with_editor(instruction_file, f"Instructions geoeffnet: {mode}")


# ── Main ──────────────────────────────────────────────────────────────────

def main() -> None:
    stdin_content = read_stdin()

    # Frage ermitteln (positional > mode-argument)
    question = args.question
    for flag in (args.codex, args.mail, args.mail_advanced, args.mail_pro, args.translate):
        if not question and isinstance(flag, str):
            question = flag

    # Piped-Input ohne Frage: nur Mail/Translate erlauben
    if stdin_content and not question:
        if not (args.mail or args.mail_advanced or args.mail_pro or args.translate):
            console.print("[red]✗ Bei Piped-Input ist eine Frage erforderlich![/red]")
            console.print("[yellow]Beispiel: cat file.txt | kit \"Zusammenfassen\"[/yellow]")
            console.print("[yellow]Mail/Translate funktionieren auch ohne Frage.[/yellow]")
            sys.exit(1)

    initial_message = None
    if stdin_content and question:
        initial_message = build_piped_message(question, stdin_content)

    # ── Routing ────────────────────────────────────────────────────────

    if args.init:
        control = input("Config auf Werkseinstellungen zuruecksetzen? (j/n): ").lower()
        if control in ("j", "y", "ja"):
            init_config()
            console.print("[green]Config zurueckgesetzt[/green]")
        else:
            console.print("[yellow]Abgebrochen[/yellow]")
        sys.exit(0)

    elif args.edit_instructions:
        edit_instructions(args.edit_instructions)
        sys.exit(0)

    elif args.setup:
        show_setup_file()

    elif args.version:
        console.print(f"[bold cyan]Version:[/bold cyan]       [green]{VERSION}[/green]")
        console.print(f"[bold cyan]Autor:[/bold cyan]        [yellow]{AUTHOR}[/yellow]")
        console.print(f"[bold cyan]Lizenz:[/bold cyan]        [blue]{LICENSE}[/blue]")
        console.print(f"[bold cyan]SDK:[/bold cyan]           [magenta]anthropic {ANTHROPIC_VERSION}[/magenta]")
        console.print(f"[bold cyan]Codex-Login:[/bold cyan]   [magenta]{auth_file()}[/magenta]")
        console.print(f"[bold cyan]Beschreibung:[/bold cyan]  [magenta]{DESCRIPTION}[/magenta]")

    elif args.mail:
        _run_mail("email", args.mail, stdin_content, question)

    elif args.mail_advanced:
        _run_mail("email_advanced", args.mail_advanced, stdin_content, question)

    elif args.mail_pro:
        _run_mail("email_pro", args.mail_pro, stdin_content, question)

    elif args.translate:
        if isinstance(args.translate, str):
            translate(args.translate)
        elif stdin_content:
            console.print("[green]Piped-Input wird verarbeitet[/green]")
            content = build_piped_message(question, stdin_content) if question else stdin_content
            translate(content)
        else:
            text = pasteline()
            console.print("[green]Aus Clipboard gelesen[/green]")
            translate(text)

    elif args.codex:
        if isinstance(args.codex, str) and not stdin_content:
            codex(args.codex)
        else:
            interactive_chat(codex, mode="codex", resume_chat=args.resume,
                             initial_message=initial_message)

    else:
        msg = initial_message if initial_message else question
        interactive_chat(normalchat, mode="normalchat", resume_chat=args.resume,
                         initial_message=msg)


def run() -> None:
    """Entry-Point (pip/pipx und Binary): main() mit sauberem Ctrl+C."""
    try:
        main()
    except KeyboardInterrupt:
        console.print("\n[yellow]Beendet[/yellow]")
        os._exit(0)


if __name__ == "__main__":
    run()
