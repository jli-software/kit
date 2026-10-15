//! Umgebung für den Terminal-Assistenten: OS, aktuelle Shell, Admin-Rechte, Ordner.
//!
//! Die Shell wird über die Elternprozesse bestimmt (kit läuft als Kind der
//! Shell), nicht über $SHELL — das ist nur die Login-Shell.

use std::fmt::Write;

/// Shells, nach denen in den Elternprozessen gesucht wird.
const SHELLS: [&str; 13] = [
    "pwsh",
    "powershell",
    "cmd",
    "bash",
    "zsh",
    "fish",
    "nu",
    "sh",
    "dash",
    "ksh",
    "tcsh",
    "csh",
    "xonsh",
];

pub struct Environment {
    pub os: String,
    pub shell: String,
    /// Sprach-Tag für Codeblöcke (powershell, bash, …)
    pub code_lang: &'static str,
    pub admin: bool,
    pub cwd: String,
}

impl Environment {
    pub fn detect() -> Self {
        let shell_exe = find_shell();
        let (shell, code_lang) = describe_shell(shell_exe.as_deref());
        Environment {
            os: os_description(),
            shell,
            code_lang,
            admin: is_admin(),
            cwd: std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        }
    }

    /// Block, der an die Instructions des Terminal-Modus angehängt wird.
    pub fn prompt_block(&self) -> String {
        let mut s = String::from("\n\n## Environment (detected by kit)\n");
        let _ = writeln!(s, "- Operating system: {}", self.os);
        let _ = writeln!(
            s,
            "- Current shell: {} (use ```{} code blocks)",
            self.shell, self.code_lang
        );
        let _ = writeln!(
            s,
            "- Session has admin/root rights: {}",
            if self.admin { "yes" } else { "no" }
        );
        let _ = writeln!(s, "- Current directory: {}", self.cwd);
        s
    }
}

/// Programmname ohne Pfad und ohne .exe, kleingeschrieben.
fn stem(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let base = base
        .strip_suffix(".exe")
        .or_else(|| base.strip_suffix(".EXE"))
        .unwrap_or(base);
    base.trim_start_matches('-').to_lowercase() // Login-Shells heissen "-zsh"
}

pub fn describe_shell(exe: Option<&str>) -> (String, &'static str) {
    let Some(exe) = exe else {
        return match std::env::var("SHELL").ok().map(|s| stem(&s)) {
            Some(login) if !login.is_empty() => {
                (format!("{login} (login shell, not verified)"), "bash")
            }
            _ if cfg!(windows) => ("PowerShell (assumed)".into(), "powershell"),
            _ => ("sh (assumed)".into(), "bash"),
        };
    };
    match stem(exe).as_str() {
        "pwsh" => ("PowerShell 7 (pwsh)".into(), "powershell"),
        "powershell" => ("Windows PowerShell 5.1".into(), "powershell"),
        "cmd" => ("Command Prompt (cmd.exe)".into(), "cmd"),
        "bash" if cfg!(windows) => ("Git Bash (bash on Windows)".into(), "bash"),
        "fish" => ("fish".into(), "fish"),
        "nu" => ("Nushell".into(), "nu"),
        other => (other.to_owned(), "bash"),
    }
}

/// Sucht in den Elternprozessen die erste bekannte Shell.
fn find_shell() -> Option<String> {
    let mut pid = std::process::id();
    for _ in 0..6 {
        let (ppid, name) = parent_of(pid)?;
        if SHELLS.contains(&stem(&name).as_str()) {
            return Some(name);
        }
        if ppid == 0 || ppid == pid {
            return None;
        }
        pid = ppid;
    }
    None
}

/// (Eltern-PID, Name des Elternprozesses)
#[cfg(target_os = "linux")]
fn parent_of(pid: u32) -> Option<(u32, String)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Format: pid (comm) state ppid … — comm kann Klammern enthalten
    let ppid: u32 = stat
        .rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let name = std::fs::read_to_string(format!("/proc/{ppid}/comm")).ok()?;
    Some((ppid, name.trim().to_owned()))
}

#[cfg(all(unix, not(target_os = "linux")))]
fn parent_of(pid: u32) -> Option<(u32, String)> {
    let ps = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("ps").args(args).output().ok()?;
        Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let ppid: u32 = ps(&["-o", "ppid=", "-p", &pid.to_string()])?.parse().ok()?;
    let name = ps(&["-o", "comm=", "-p", &ppid.to_string()])?;
    Some((ppid, name))
}

#[cfg(windows)]
fn parent_of(pid: u32) -> Option<(u32, String)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    // SAFETY: Snapshot wird nur gelesen und am Ende geschlossen
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut procs: Vec<(u32, u32, String)> = Vec::new();
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry) != 0;
        while ok {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
            procs.push((entry.th32ProcessID, entry.th32ParentProcessID, name));
            ok = Process32NextW(snap, &mut entry) != 0;
        }
        CloseHandle(snap);

        let ppid = procs.iter().find(|p| p.0 == pid)?.1;
        let name = procs.iter().find(|p| p.0 == ppid)?.2.clone();
        Some((ppid, name))
    }
}

#[cfg(windows)]
fn is_admin() -> bool {
    // SAFETY: einfache Abfrage ohne Parameter
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

#[cfg(unix)]
fn is_admin() -> bool {
    // SAFETY: geteuid hat keine Vorbedingungen
    unsafe { libc::geteuid() == 0 }
}

#[cfg(windows)]
fn os_description() -> String {
    // Win32-Registry über reg.exe wäre langsam; Werte direkt lesen
    let product = reg_string("ProductName").unwrap_or_else(|| "Windows".into());
    let display = reg_string("DisplayVersion").unwrap_or_default();
    let build: u32 = reg_string("CurrentBuild")
        .and_then(|b| b.parse().ok())
        .unwrap_or(0);
    // ProductName sagt auch auf Windows 11 "Windows 10 …"
    let product = if build >= 22000 {
        product.replacen("Windows 10", "Windows 11", 1)
    } else {
        product
    };
    format!("{product} {display} (build {build})").replace("  ", " ")
}

#[cfg(windows)]
fn reg_string(value: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let key = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
    let name = wide(value);
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: Puffer und Grösse passen zusammen; String ist nullterminiert
    let rc = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if rc != 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

#[cfg(target_os = "linux")]
fn os_description() -> String {
    let pretty = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|t| {
            t.lines().find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_owned())
            })
        })
        .unwrap_or_else(|| "Linux".into());
    let wsl = if crate::platform::is_wsl() {
        " (WSL on Windows)"
    } else {
        ""
    };
    let termux = if crate::platform::is_termux() {
        " (Termux on Android)"
    } else {
        ""
    };
    format!("{pretty}{wsl}{termux}")
}

#[cfg(target_os = "macos")]
fn os_description() -> String {
    let version = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    format!("macOS {version}")
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn os_description() -> String {
    std::env::consts::OS.to_owned()
}

/// Erster Codeblock einer Antwort (für die Zwischenablage), ohne Zäune.
pub fn first_code_block(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let after_fence = &text[start + 3..];
    let body = &after_fence[after_fence.find('\n')? + 1..];
    let end = body.find("```")?;
    let code = body[..end].trim_end();
    (!code.trim().is_empty()).then(|| code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_names() {
        assert_eq!(
            describe_shell(Some(r"C:\Program Files\PowerShell\7\pwsh.exe")).1,
            "powershell"
        );
        assert_eq!(
            describe_shell(Some("powershell.exe")).0,
            "Windows PowerShell 5.1"
        );
        assert_eq!(describe_shell(Some("cmd.exe")).1, "cmd");
        assert_eq!(describe_shell(Some("-zsh")).0, "zsh");
        assert_eq!(describe_shell(Some("fish")).1, "fish");
    }

    #[test]
    fn code_block_for_clipboard() {
        let answer = "Neustart:\n\n```powershell\nRestart-Computer\n```\n\nBraucht Admin.";
        assert_eq!(
            first_code_block(answer).as_deref(),
            Some("Restart-Computer")
        );
        assert_eq!(first_code_block("kein Code"), None);
        let two = "```bash\nsudo reboot\n```\nOder:\n```bash\nshutdown -r now\n```";
        assert_eq!(first_code_block(two).as_deref(), Some("sudo reboot"));
    }

    #[test]
    fn detects_something() {
        let env = Environment::detect();
        assert!(!env.os.is_empty());
        assert!(env.prompt_block().contains("Current shell:"));
    }
}
