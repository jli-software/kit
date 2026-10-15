//! Terminal-Ausgabe: Markdown wird beim Streamen formatiert, Zeichen für Zeichen.
//!
//! Bewusst klein gehalten: Überschriften, Listen, Zitate, Codeblöcke,
//! **fett** und `code`. Alles andere (Tabellen, Links) bleibt Rohtext.
//! Ohne Terminal oder mit NO_COLOR wird der Text unverändert ausgegeben.

use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub const RESET: &str = "\x1b[0m";
pub const DIM: &str = "\x1b[2m";
pub const BOLD: &str = "\x1b[1m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const CYAN: &str = "\x1b[36m";
const CODE: &str = "\x1b[96m";
const HEADING: &str = "\x1b[1;35m";

pub fn color_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

/// Farbiger Text für Statusmeldungen (ohne Farbe, wenn nicht erlaubt).
pub fn paint(style: &str, text: &str) -> String {
    if color_enabled() {
        format!("{style}{text}{RESET}")
    } else {
        text.to_owned()
    }
}

pub fn error(msg: &str) {
    eprintln!("{}", paint(RED, &format!("✗ {msg}")));
}

#[derive(Default)]
pub struct Markdown {
    /// Zeichen am Zeilenanfang, bis klar ist, was die Zeile ist
    pending: String,
    line_start: bool,
    in_code_block: bool,
    line_style: &'static str,
    bold: bool,
    inline_code: bool,
    star_pending: bool,
    out: String,
}

impl Markdown {
    pub fn new() -> Self {
        Markdown {
            line_start: true,
            ..Default::default()
        }
    }

    /// Verarbeitet ein Stück Text und gibt die fertige Terminal-Ausgabe zurück.
    pub fn push(&mut self, text: &str) -> String {
        for c in text.chars() {
            self.feed(c);
        }
        std::mem::take(&mut self.out)
    }

    /// Schliesst die Ausgabe ab (offene Zeile, Farben zurücksetzen).
    pub fn finish(&mut self) -> String {
        if self.line_start && !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            self.start_line(&pending);
        }
        if self.star_pending {
            self.star_pending = false;
            self.out.push('*');
        }
        self.out.push_str(RESET);
        std::mem::take(&mut self.out)
    }

    fn feed(&mut self, c: char) {
        if self.line_start {
            self.pending.push(c);
            if c != '\n' && self.prefix_undecided() {
                return;
            }
            let pending = std::mem::take(&mut self.pending);
            self.start_line(&pending);
            return;
        }
        self.inline(c);
    }

    /// Kann der Zeilenanfang noch ein Markdown-Präfix werden?
    fn prefix_undecided(&self) -> bool {
        let t = self.pending.trim_start();
        if t.is_empty() {
            return true;
        }
        if "```".starts_with(t) && t.len() < 3 {
            return true;
        }
        if t.starts_with("```") {
            return !self.in_code_block; // Fence-Zeile bis zum Zeilenende sammeln
        }
        if self.in_code_block {
            return false;
        }
        (t.len() <= 6 && t.chars().all(|c| c == '#')) || matches!(t, "-" | "*" | "+" | ">")
    }

    fn start_line(&mut self, pending: &str) {
        self.line_start = false;
        let indent_len = pending.len() - pending.trim_start_matches([' ', '\t']).len();
        let (indent, rest) = pending.split_at(indent_len);

        if rest.starts_with("```") {
            self.in_code_block = !self.in_code_block;
            self.out.push_str(DIM);
            self.out.push_str(pending.trim_end_matches('\n'));
            self.out.push_str(RESET);
            if pending.ends_with('\n') {
                self.end_line();
            }
            return;
        }
        if self.in_code_block {
            self.out.push_str(CODE);
            self.out.push_str(pending);
            if pending.ends_with('\n') {
                self.out.push_str(RESET);
                self.line_start = true;
            }
            return;
        }

        self.out.push_str(indent);
        let hashes = rest.chars().take_while(|&c| c == '#').count();
        let body = if hashes > 0 && hashes <= 6 && rest[hashes..].starts_with(' ') {
            self.line_style = HEADING;
            self.style();
            &rest[hashes + 1..]
        } else if let Some(b) = ["- ", "* ", "+ "].iter().find_map(|p| rest.strip_prefix(p)) {
            self.out.push_str("• ");
            b
        } else if let Some(b) = rest.strip_prefix("> ") {
            self.line_style = DIM;
            self.out.push_str(DIM);
            self.out.push_str("│ ");
            b
        } else {
            rest
        };
        for c in body.chars() {
            self.inline(c);
        }
    }

    fn inline(&mut self, c: char) {
        if self.in_code_block {
            self.out.push(c);
            if c == '\n' {
                self.out.push_str(RESET);
                self.line_start = true;
            }
            return;
        }
        if self.star_pending {
            self.star_pending = false;
            if c == '*' {
                self.bold = !self.bold;
                self.style();
                return;
            }
            self.out.push('*');
        }
        match c {
            '\n' => self.end_line(),
            '*' if !self.inline_code => self.star_pending = true,
            '`' => {
                self.inline_code = !self.inline_code;
                self.style();
            }
            _ => self.out.push(c),
        }
    }

    fn end_line(&mut self) {
        self.bold = false;
        self.inline_code = false;
        self.line_style = "";
        self.out.push_str(RESET);
        self.out.push('\n');
        self.line_start = true;
    }

    fn style(&mut self) {
        self.out.push_str(RESET);
        self.out.push_str(self.line_style);
        if self.bold {
            self.out.push_str(BOLD);
        }
        if self.inline_code {
            self.out.push_str(CODE);
        }
    }
}

/// Streamt Text auf stdout — formatiert im Terminal, roh sonst.
pub struct Printer {
    md: Option<Markdown>,
}

impl Printer {
    pub fn new() -> Self {
        Printer {
            md: color_enabled().then(Markdown::new),
        }
    }

    pub fn print(&mut self, text: &str) {
        let rendered = match &mut self.md {
            Some(md) => md.push(text),
            None => text.to_owned(),
        };
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(rendered.as_bytes());
        let _ = out.flush();
    }

    pub fn finish(&mut self, ends_with_newline: bool) {
        let mut rest = self.md.as_mut().map(Markdown::finish).unwrap_or_default();
        if !ends_with_newline {
            rest.push('\n');
        }
        print!("{rest}");
        let _ = std::io::stdout().flush();
    }
}

/// Spinner auf stderr, bis `stop()` aufgerufen wird.
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    pub fn start(label: String) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let handle = std::io::stderr().is_terminal().then(|| {
            let running = running.clone();
            thread::spawn(move || {
                let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                let mut i = 0;
                while running.load(Ordering::SeqCst) {
                    eprint!("\r{GREEN}{} {label}{RESET}", frames[i % frames.len()]);
                    i += 1;
                    thread::sleep(Duration::from_millis(80));
                }
                eprint!("\r\x1b[2K");
            })
        });
        Spinner { running, handle }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(chunks: &[&str]) -> String {
        let mut md = Markdown::new();
        let mut out: String = chunks.iter().map(|c| md.push(c)).collect();
        out.push_str(&md.finish());
        // ANSI-Codes entfernen → nur der sichtbare Text
        let mut plain = String::new();
        let mut esc = false;
        for c in out.chars() {
            match (esc, c) {
                (false, '\x1b') => esc = true,
                (true, 'm') => esc = false,
                (true, _) => {}
                (false, c) => plain.push(c),
            }
        }
        plain
    }

    #[test]
    fn heading_bullets_and_bold() {
        let text = render(&["# Tit", "el\n- eins **fe", "tt**\n* zwei\n"]);
        assert_eq!(text, "Titel\n• eins fett\n• zwei\n");
    }

    #[test]
    fn code_block_kept_verbatim() {
        let text = render(&["```py\nx = 2 * 3 ** 2\n", "```\nfertig"]);
        assert_eq!(text, "```py\nx = 2 * 3 ** 2\n```\nfertig");
    }

    #[test]
    fn single_star_and_rule_stay() {
        assert_eq!(render(&["2 * 3\n---\n"]), "2 * 3\n---\n");
    }

    #[test]
    fn inline_code_and_quote() {
        assert_eq!(render(&["> Zitat `a*b`\n"]), "│ Zitat a*b\n");
    }
}
