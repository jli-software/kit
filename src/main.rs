//! kit — KIterminal: schlankes KI-CLI-Tool für schnelle Aufgaben.
//!
//! Modi: Chat, Codex (Programmier-Assistent), Mail (-m/-ma/-mp), Übersetzung (-t)
//! Provider: OpenAI Codex (ChatGPT-Login), Anthropic, DeepSeek

mod codex_auth;
mod config;
mod history;
mod http;
mod interrupt;
mod platform;
mod providers;
mod render;

use std::borrow::Cow;
use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::{Completer, Helper, Hinter, Validator};

use config::Config;
use providers::{ApiError, Message};
use render::{CYAN, DIM, GREEN, YELLOW, paint};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIPTION: &str = "KIterminal — Schlankes KI-CLI-Tool. Provider: OpenAI Codex (ChatGPT-Login), Anthropic, DeepSeek.";

const EPILOG: &str = "\
Beispiele:
  kit                     Chat (default)
  kit \"Frage\"             Chat mit erster Frage
  kit -c                  Codex Programmier-Assistent
  kit -c \"Frage\"          Codex One-Shot
  kit -m                  E-Mail-Korrektur (aus Clipboard)
  kit -m \"Text\"           E-Mail-Korrektur (direkt)
  kit -ma \"Text\"          E-Mail-Korrektur + Verbesserung
  kit -mp \"Text\"          Professioneller E-Mail-Rewrite
  kit -t \"Hello World\"    Übersetzung DE<->EN
  kit -r                  Chat fortsetzen
  kit -r -c               Codex-Chat fortsetzen

Piped Input:
  cat file.txt | kit \"Zusammenfassen\"
  cat script.py | kit -c \"Erklären\"
  cat email.txt | kit -m
  cat doc.txt | kit -t

Konfiguration:
  kit -s                  Config-Datei bearbeiten
  kit -e codex            Instructions eines Modus bearbeiten
                          (normalchat, codex, email, email-advanced, email-pro, translate)
  Instruction-Dateien:    ~/.config/KIterminal/instructions/
  Config-Datei:           ~/.config/KIterminal/config.json

Provider-Login:
  codex                   ChatGPT-Abo, einmal 'codex login' (Codex CLI)
  ANTHROPIC_API_KEY       Anthropic API-Key
  DEEPSEEK_API_KEY        DeepSeek API-Key

Mehr Infos: https://github.com/jli-software/kit";

#[derive(Parser)]
#[command(name = "kit", about = DESCRIPTION, after_help = EPILOG, disable_version_flag = true)]
struct Cli {
    /// Codex Programmier-Assistent (interaktiv oder One-Shot)
    #[arg(short = 'c', long = "codex", num_args = 0..=1, default_missing_value = "", value_name = "TEXT", help_heading = "KI-Modi")]
    codex: Option<String>,

    /// Übersetzung DE<->EN (Clipboard oder Text)
    #[arg(short = 't', long, num_args = 0..=1, default_missing_value = "", value_name = "TEXT", help_heading = "Textverarbeitung")]
    translate: Option<String>,

    /// E-Mail-Korrektur (Clipboard oder Text)
    #[arg(short = 'm', long = "mail", num_args = 0..=1, default_missing_value = "", value_name = "TEXT", help_heading = "Textverarbeitung")]
    mail: Option<String>,

    /// [-ma] E-Mail-Korrektur + inhaltliche Verbesserung
    #[arg(long = "mail-advanced", num_args = 0..=1, default_missing_value = "", value_name = "TEXT", help_heading = "Textverarbeitung")]
    mail_advanced: Option<String>,

    /// [-mp] Professioneller E-Mail-Rewrite (Business-Ton)
    #[arg(long = "mail-pro", num_args = 0..=1, default_missing_value = "", value_name = "TEXT", help_heading = "Textverarbeitung")]
    mail_pro: Option<String>,

    /// Vorherigen Chat fortsetzen
    #[arg(short = 'r', long, help_heading = "Chat-Management")]
    resume: bool,

    /// Config-Datei mit Editor öffnen
    #[arg(short = 's', long, help_heading = "Konfiguration")]
    setup: bool,

    /// Config auf Werkseinstellungen zurücksetzen
    #[arg(short = 'i', long, help_heading = "Konfiguration")]
    init: bool,

    /// Instructions für einen Modus bearbeiten
    #[arg(
        short = 'e',
        long = "edit-instructions",
        value_name = "MODE",
        help_heading = "Konfiguration"
    )]
    edit_instructions: Option<String>,

    /// Version & Info anzeigen
    #[arg(short = 'v', long, help_heading = "Information")]
    version: bool,

    /// Frage (erforderlich bei Piped-Input)
    question: Option<String>,
}

/// `-ma`/`-mp` sind zweistellige Kurzflags (wie in der Python-Version) — clap kennt
/// nur einstellige, darum werden sie vorab auf die Langform umgeschrieben.
fn normalize_args(args: impl Iterator<Item = String>) -> Vec<String> {
    args.map(|a| match a.as_str() {
        "-ma" => "--mail-advanced".into(),
        "-mp" => "--mail-pro".into(),
        _ => a,
    })
    .collect()
}

fn build_piped_message(question: &str, stdin: &str) -> String {
    format!("{question}\n\n```\n{stdin}\n```")
}

fn read_stdin() -> Option<String> {
    if std::io::stdin().is_terminal() {
        return None;
    }
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).ok()?;
    let text = buf.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

// ── App ───────────────────────────────────────────────────────────────────

struct App {
    cfg: Config,
}

enum Turn {
    Ok,
    Interrupted,
}

impl App {
    fn system(&self, mode: &str) -> String {
        let text = config::instruction(mode, &self.cfg.language);
        if mode == "codex" {
            text.replace("{currentOS}", os_name())
                .replace("{platforminfo}", &platform_info())
        } else {
            text
        }
    }

    /// Führt einen Call aus. `live` = Markdown-Streaming, sonst Spinner + fertiger Text.
    fn call(&self, mode: &str, messages: &[Message], live: bool) -> Result<String, String> {
        let mc = self.cfg.mode(mode);
        let system = self.system(mode);
        let mut full = String::new();
        let mut spinner = Some(render::Spinner::start(format!(
            "{}/{} …",
            providers::display_name(&mc.provider),
            mc.model
        )));
        let mut printer = render::Printer::new();

        let result = providers::stream_text(&mc, &system, messages, &mut |text| {
            if live {
                if let Some(mut s) = spinner.take() {
                    s.stop();
                }
                printer.print(text);
            }
            full.push_str(text);
        });
        if let Some(mut s) = spinner.take() {
            s.stop();
        }
        if live && !full.is_empty() {
            printer.finish(full.ends_with('\n'));
        }

        match result {
            Ok(()) => Ok(full),
            Err(ApiError::Setup(msg)) => {
                render::error(&msg);
                std::process::exit(1);
            }
            Err(ApiError::Call(msg)) => Err(msg),
        }
    }

    /// Eine Chat-Runde: Frage anhängen, Antwort streamen, Antwort ins Clipboard.
    fn chat_turn(&self, mode: &str, history: &mut Vec<Message>, question: &str) -> Turn {
        history.push(Message::user(question));
        match self.call(mode, history, true) {
            Ok(answer) => {
                platform::copy(&answer);
                history.push(Message::assistant(answer));
            }
            Err(msg) => {
                render::error(&msg);
                history.pop();
            }
        }
        if interrupt::requested() {
            Turn::Interrupted
        } else {
            Turn::Ok
        }
    }

    /// One-Shot für Mail/Übersetzung: Ergebnis drucken und ins Clipboard.
    fn one_shot(&self, mode: &str, prompt: String) {
        match self.call(mode, &[Message::user(prompt)], false) {
            Ok(text) => {
                platform::copy(&text);
                println!("{text}");
            }
            Err(msg) => render::error(&msg),
        }
    }

    fn mail(&self, mode: &str, flag_text: &str, stdin: Option<&str>) {
        let text = match (flag_text.is_empty(), stdin) {
            (false, Some(input)) => build_piped_message(flag_text, input),
            (false, None) => flag_text.to_owned(),
            (true, Some(input)) => {
                println!("{}", paint(GREEN, "Piped-Input wird verarbeitet"));
                input.to_owned()
            }
            (true, None) => {
                let text = platform::paste().unwrap_or_else(|e| {
                    render::error(&e);
                    std::process::exit(1)
                });
                println!("{}", paint(GREEN, "Aus Clipboard gelesen"));
                text
            }
        };
        let prompt = if mode == "translate" {
            format!("Translate:\n\n{text}")
        } else {
            format!("Proofread this email:\n\n{text}")
        };
        self.one_shot(mode, prompt);
    }

    fn interactive(&self, mode: &str, resume: bool, initial: Option<String>, piped: bool) {
        let mc = self.cfg.mode(mode);
        let short_model = mc
            .model
            .trim_start_matches("claude-")
            .trim_start_matches("deepseek-");
        let label = format!(
            "{}/{short_model} >",
            mc.provider.chars().take(4).collect::<String>()
        );

        let mut history: Vec<Message> = Vec::new();
        let mut chat_file: Option<PathBuf> = None;
        if let Some(path) = resume.then(|| history::select(&self.cfg, mode)).flatten() {
            match history::load(&path) {
                Ok(h) => {
                    println!(
                        "{}",
                        paint(GREEN, &format!("Chat geladen ({} Nachrichten)", h.len()))
                    );
                    history = h;
                    chat_file = Some(path);
                }
                Err(e) => {
                    render::error(&format!("Fehler beim Laden: {e}"));
                    return;
                }
            }
        }

        if initial.is_none() {
            let mode_name = if mode == "codex" { "Codex" } else { "Chat" };
            println!(
                "{}",
                paint(
                    DIM,
                    &format!(
                        "kit v{VERSION}  ·  {}  ·  {}  ·  {mode_name}",
                        providers::display_name(&mc.provider),
                        mc.model
                    )
                )
            );
        }

        let save = |history: &[Message], chat_file: &mut Option<PathBuf>| {
            if let Some(path) = history::save(&self.cfg, history, mode, chat_file.as_deref()) {
                *chat_file = Some(path);
            }
        };

        let mut interrupted = false;
        if let Some(first) = initial {
            if piped {
                println!("{}", paint(DIM, "Piped-Input wird verarbeitet ..."));
            }
            interrupted = matches!(
                self.chat_turn(mode, &mut history, &first),
                Turn::Interrupted
            );
            if piped && !interrupted {
                if !platform::reopen_stdin_to_terminal() {
                    save(&history, &mut chat_file);
                    println!(
                        "{}",
                        paint(DIM, "Tipp: Mit -r kannst du diesen Chat später fortsetzen")
                    );
                    return;
                }
                println!("{}", paint(DIM, &format!("Interaktiver Modus  —  {label}")));
            }
        }

        if !interrupted {
            let mut editor =
                match rustyline::Editor::<PromptColor, rustyline::history::DefaultHistory>::new() {
                    Ok(e) => e,
                    Err(e) => {
                        render::error(&format!("Terminal nicht verfügbar: {e}"));
                        save(&history, &mut chat_file);
                        return;
                    }
                };
            editor.set_helper(Some(PromptColor));
            loop {
                match editor.readline(&format!("{label} ")) {
                    Ok(line) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if line.eq_ignore_ascii_case("exit") {
                            break;
                        }
                        let _ = editor.add_history_entry(line);
                        if let Turn::Interrupted = self.chat_turn(mode, &mut history, line) {
                            interrupted = true;
                            break;
                        }
                    }
                    Err(ReadlineError::Interrupted) => {
                        interrupted = true;
                        break;
                    }
                    Err(_) => break, // Ctrl+D / EOF
                }
            }
        }

        if !history.is_empty() {
            save(&history, &mut chat_file);
            println!("{}", paint(DIM, "Chat gespeichert"));
        }
        if interrupted {
            println!("{}", paint(YELLOW, "Beendet"));
        }
    }
}

/// Prompt in Cyan (über den Highlighter, damit rustyline die Breite richtig misst)
#[derive(Completer, Helper, Hinter, Validator)]
struct PromptColor;

impl Highlighter for PromptColor {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default && render::color_enabled() {
            Cow::Owned(format!("\x1b[1;96m{prompt}\x1b[0m"))
        } else {
            Cow::Borrowed(prompt)
        }
    }
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "Darwin",
        "linux" => "Linux",
        other => other,
    }
}

fn platform_info() -> String {
    format!("{}-{}", os_name(), std::env::consts::ARCH)
}

fn print_version() {
    let row = |k: &str, v: &str| println!("{} {v}", paint(CYAN, &format!("{k:<14}")));
    row("Version:", VERSION);
    row("Autor:", "JLI-Software");
    row("Lizenz:", "MIT");
    row(
        "Codex-Login:",
        &codex_auth::auth_file().display().to_string(),
    );
    row("Config:", &config::config_file().display().to_string());
    row("Beschreibung:", DESCRIPTION);
}

fn edit_instructions(mode: &str) -> Result<(), String> {
    let file = match mode.to_lowercase().as_str() {
        "normalchat" => "normalchat.txt",
        "codex" => "codex.txt",
        "email" => "email.txt",
        "email-advanced" | "email_advanced" => "email_advanced.txt",
        "email-pro" | "email_pro" => "email_pro.txt",
        "translate" => "translate.txt",
        _ => {
            return Err(format!(
                "Unbekannter Modus: {mode}\nVerfügbar: codex, email, email-advanced, email-pro, normalchat, translate"
            ));
        }
    };
    platform::open_in_editor(&config::instructions_dir().join(file))?;
    println!(
        "{}",
        paint(GREEN, &format!("Instructions geöffnet: {mode}"))
    );
    Ok(())
}

fn run(cli: Cli) -> Result<(), String> {
    if cli.version {
        print_version();
        return Ok(());
    }
    if cli.init {
        print!("Config auf Werkseinstellungen zurücksetzen? (j/n): ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        if matches!(answer.trim().to_lowercase().as_str(), "j" | "y" | "ja") {
            config::reset()?;
            println!("{}", paint(GREEN, "Config zurückgesetzt"));
        } else {
            println!("{}", paint(YELLOW, "Abgebrochen"));
        }
        return Ok(());
    }
    if let Some(mode) = &cli.edit_instructions {
        config::ensure_files()?;
        return edit_instructions(mode);
    }
    if cli.setup {
        config::ensure_files()?;
        platform::open_in_editor(&config::config_file())?;
        println!("{}", paint(GREEN, "Config-Datei geöffnet"));
        return Ok(());
    }

    let app = App {
        cfg: config::load()?,
    };
    let stdin = read_stdin();
    let one_shot_flag = cli.mail.is_some()
        || cli.mail_advanced.is_some()
        || cli.mail_pro.is_some()
        || cli.translate.is_some();

    // Frage: Positional vor Text eines Modus-Flags
    let question = cli.question.clone().or_else(|| {
        [
            &cli.codex,
            &cli.mail,
            &cli.mail_advanced,
            &cli.mail_pro,
            &cli.translate,
        ]
        .into_iter()
        .flatten()
        .find(|t| !t.is_empty())
        .cloned()
    });

    if stdin.is_some() && question.is_none() && !one_shot_flag {
        return Err("Bei Piped-Input ist eine Frage erforderlich!\nBeispiel: cat file.txt | kit \"Zusammenfassen\"\nMail/Translate funktionieren auch ohne Frage.".into());
    }

    let mail_text = |flag: &Option<String>| -> String {
        // Text am Flag hat Vorrang; sonst gilt eine Positional-Frage als Text
        flag.clone()
            .filter(|t| !t.is_empty())
            .or_else(|| cli.question.clone())
            .unwrap_or_default()
    };

    if cli.mail.is_some() {
        app.mail("email", &mail_text(&cli.mail), stdin.as_deref());
    } else if cli.mail_advanced.is_some() {
        app.mail(
            "email_advanced",
            &mail_text(&cli.mail_advanced),
            stdin.as_deref(),
        );
    } else if cli.mail_pro.is_some() {
        app.mail("email_pro", &mail_text(&cli.mail_pro), stdin.as_deref());
    } else if cli.translate.is_some() {
        app.mail("translate", &mail_text(&cli.translate), stdin.as_deref());
    } else if let Some(text) = &cli.codex {
        if !text.is_empty() && stdin.is_none() {
            let mut history = Vec::new();
            app.chat_turn("codex", &mut history, text);
        } else {
            let initial = stdin
                .as_deref()
                .map(|s| build_piped_message(question.as_deref().unwrap_or(""), s));
            app.interactive("codex", cli.resume, initial, stdin.is_some());
        }
    } else {
        let initial = match &stdin {
            Some(s) => Some(build_piped_message(question.as_deref().unwrap_or(""), s)),
            None => question,
        };
        app.interactive("normalchat", cli.resume, initial, stdin.is_some());
    }
    Ok(())
}

fn main() -> ExitCode {
    #[cfg(windows)]
    let _ = enable_ansi_support::enable_ansi_support();
    interrupt::install();

    let cli = Cli::parse_from(normalize_args(std::env::args()));
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            render::error(&msg);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse_from(normalize_args(
            std::iter::once("kit")
                .chain(args.iter().copied())
                .map(String::from),
        ))
    }

    #[test]
    fn two_letter_mail_flags() {
        let cli = parse(&["-ma", "Text"]);
        assert_eq!(cli.mail_advanced.as_deref(), Some("Text"));
        assert!(cli.mail.is_none());
        assert_eq!(parse(&["-mp"]).mail_pro.as_deref(), Some(""));
    }

    #[test]
    fn flag_without_text_and_question() {
        let cli = parse(&["-m"]);
        assert_eq!(cli.mail.as_deref(), Some(""));
        let cli = parse(&["Wie spät?"]);
        assert_eq!(cli.question.as_deref(), Some("Wie spät?"));
        let cli = parse(&["-r", "-c"]);
        assert!(cli.resume && cli.codex.as_deref() == Some(""));
    }

    #[test]
    fn piped_message_format() {
        assert_eq!(build_piped_message("Q", "x"), "Q\n\n```\nx\n```");
    }
}
