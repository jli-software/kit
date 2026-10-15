//! Plattform-Helfer: Zwischenablage, Editor, Terminal nach Piped-Input.

#[cfg(unix)]
use std::io::Write;
use std::path::Path;
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;

#[cfg(all(unix, not(target_os = "macos")))]
pub fn is_wsl() -> bool {
    cfg!(target_os = "linux")
        && std::fs::read_to_string("/proc/version")
            .map(|v| {
                let v = v.to_lowercase();
                v.contains("microsoft") || v.contains("wsl")
            })
            .unwrap_or(false)
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn is_termux() -> bool {
    Path::new("/data/data/com.termux").exists()
}

fn has_cmd(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        dir.join(name).is_file() || (cfg!(windows) && dir.join(format!("{name}.exe")).is_file())
    })
}

#[cfg(unix)]
/// Text an ein Programm übergeben (stdin). Fehler werden ignoriert.
fn pipe_to(cmd: &str, args: &[&str], text: &str) -> bool {
    let Ok(mut child) = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

#[cfg(unix)]
fn output_of(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd)
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(all(unix, not(target_os = "macos")))]
type ClipCmd = (&'static str, &'static [&'static str]);

/// Linux: Programm-Paare (kopieren, einfügen) je nach Umgebung
#[cfg(all(unix, not(target_os = "macos")))]
fn linux_clipboard() -> Option<(ClipCmd, ClipCmd)> {
    if is_termux() {
        return Some((("termux-clipboard-set", &[]), ("termux-clipboard-get", &[])));
    }
    if is_wsl() {
        return Some((
            ("clip.exe", &[]),
            (
                "powershell.exe",
                &["-NoProfile", "-Command", "Get-Clipboard"],
            ),
        ));
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && has_cmd("wl-copy") {
        return Some((("wl-copy", &[]), ("wl-paste", &["--no-newline"])));
    }
    if has_cmd("xclip") {
        return Some((
            ("xclip", &["-selection", "clipboard"]),
            ("xclip", &["-selection", "clipboard", "-o"]),
        ));
    }
    if has_cmd("xsel") {
        return Some((
            ("xsel", &["--clipboard", "--input"]),
            ("xsel", &["--clipboard", "--output"]),
        ));
    }
    None
}

/// Text ins Clipboard kopieren (still, Fehler werden ignoriert).
pub fn copy(text: &str) {
    #[cfg(windows)]
    {
        let _ = clipboard_win::set_clipboard_string(text);
    }
    #[cfg(target_os = "macos")]
    {
        pipe_to("pbcopy", &[], text);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(((cmd, args), _)) = linux_clipboard() {
            pipe_to(cmd, args, text);
        }
    }
}

/// Text aus dem Clipboard lesen.
pub fn paste() -> Result<String, String> {
    #[cfg(windows)]
    {
        clipboard_win::get_clipboard_string().map_err(|e| format!("Clipboard-Fehler: {e}"))
    }
    #[cfg(target_os = "macos")]
    {
        output_of("pbpaste", &[]).ok_or_else(|| "Clipboard-Fehler: pbpaste".into())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let (_, (cmd, args)) = linux_clipboard().ok_or(
            "Kein Clipboard-Programm gefunden (wl-clipboard, xclip oder xsel installieren)",
        )?;
        output_of(cmd, args)
            .map(|t| if is_wsl() { t.trim().to_owned() } else { t })
            .ok_or_else(|| format!("Clipboard-Fehler: {cmd}"))
    }
}

/// Öffnet eine Datei im Editor ($EDITOR, sonst System-Standard).
pub fn open_in_editor(path: &Path) -> Result<(), String> {
    let path_str = path.to_string_lossy().into_owned();
    let status = if let Some(editor) = std::env::var("EDITOR").ok().filter(|e| !e.is_empty()) {
        Command::new(editor).arg(&path_str).status()
    } else if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "start", "", &path_str])
            .status()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(&path_str).status()
    } else if has_cmd("nvim") {
        Command::new("nvim").arg(&path_str).status()
    } else {
        Command::new("xdg-open").arg(&path_str).status()
    };
    status
        .map(|_| ())
        .map_err(|e| format!("Fehler beim Öffnen: {e}"))
}

/// Nach Piped-Input stdin wieder aufs Terminal legen (für Folgefragen).
pub fn reopen_stdin_to_terminal() -> bool {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let Ok(tty) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
        else {
            return false;
        };
        // SAFETY: dup2 auf gültige Deskriptoren; tty bleibt bis dahin offen
        unsafe { libc::dup2(tty.as_raw_fd(), 0) >= 0 }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::IntoRawHandle;
        use windows_sys::Win32::System::Console::{STD_INPUT_HANDLE, SetStdHandle};
        let Ok(con) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("CONIN$")
        else {
            return false;
        };
        // SAFETY: Handle gehört ab jetzt dem Prozess als Standard-Eingabe
        unsafe { SetStdHandle(STD_INPUT_HANDLE, con.into_raw_handle() as _) != 0 }
    }
}
