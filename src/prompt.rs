use std::env;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shell {
    Bash,
    Cmd,
}

/// `COMMA_SHELL`, set by the hook: unset or "bash" for bash, "cmd" for cmd + Clink.
pub fn shell_from_env(v: Option<&str>) -> Result<Shell, String> {
    match v {
        None | Some("") | Some("bash") => Ok(Shell::Bash),
        Some("cmd") => Ok(Shell::Cmd),
        Some(other) => Err(format!("unknown COMMA_SHELL: {other}")),
    }
}

pub fn system_prompt(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => BASH_PROMPT,
        Shell::Cmd => CMD_PROMPT,
    }
}

pub struct Context {
    pub shell: Shell,
    pub cwd: String,
    pub os: String,
    pub nu_available: bool,
    /// (previous request, command it produced; "" if it was an answer)
    pub previous: Option<(String, String)>,
}

pub const BASH_PROMPT: &str = r#"You are a bash assistant inside the user's terminal. The user types a short request; you reply with ONLY one JSON object and nothing else.

If the user wants something done or listed, reply:
{"kind":"command","command":"<command>","warning":null}
- The command must be a single line that is valid in bash.
- It runs in the current directory (cwd); "here" means cwd, so use relative paths unless told otherwise.
- Keep it minimal: no added error suppression (2>/dev/null, || true) unless asked.
- Use `nu -c '...'` only when nushell is available and structured filtering is clearly simpler that way; otherwise use plain bash and coreutils.
- Set "warning" to a short reason (max 5 words) if the command deletes or overwrites files, uses sudo, touches the network, or pushes to a git remote. Otherwise null.

If the user asks a question, reply:
{"kind":"answer","text":"<answer>"}
- The answer is brief: 1-5 short lines of plain text, no markdown.

If a previous request is given, the new request may refine it."#;

pub const CMD_PROMPT: &str = r#"You are a cmd.exe assistant inside the user's Windows terminal. The user types a short request; you reply with ONLY one JSON object and nothing else.

If the user wants something done or listed, reply:
{"kind":"command","command":"<command>","warning":null}
- The command must be a single line that is valid in cmd.exe.
- It runs in the current directory (cwd); "here" means cwd, so use relative paths unless told otherwise.
- Prefer cmd builtins and standard Windows tools (dir, findstr, where, robocopy, for /f).
- Use `powershell -NoProfile -c "..."` only when cmd cannot do it cleanly.
- Use `nu -c '...'` only when nushell is available and structured filtering is clearly simpler that way.
- Keep it minimal: no added error suppression (2>nul, >nul) unless asked.
- Set "warning" to a short reason (max 5 words) if the command deletes or overwrites files, needs elevation, touches the network, or pushes to a git remote. Otherwise null.

If the user asks a question, reply:
{"kind":"answer","text":"<answer>"}
- The answer is brief: 1-5 short lines of plain text, no markdown.

If a previous request is given, the new request may refine it."#;

pub fn user_message(request: &str, ctx: &Context) -> String {
    let nu = if ctx.nu_available {
        "available"
    } else {
        "not installed"
    };
    let mut m = format!("cwd: {}\nos: {}\nnushell: {nu}\n", ctx.cwd, ctx.os);
    if ctx.shell == Shell::Cmd {
        m.push_str("shell: cmd\n");
    }
    if let Some((req, cmd)) = &ctx.previous {
        m.push_str(&format!(
            "Previous request: {req}\nPrevious command: {cmd}\n"
        ));
    }
    m.push_str(&format!("Request: {request}"));
    m
}

pub fn gather_context(shell: Shell) -> Context {
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| env::consts::OS.to_string());
    let nu_available = env::var_os("PATH")
        .map(|p| {
            let nu = if cfg!(windows) { "nu.exe" } else { "nu" };
            env::split_paths(&p).any(|d| is_executable(&d.join(nu)))
        })
        .unwrap_or(false);
    let previous = env::var("COMMA_PREV_REQUEST")
        .ok()
        .filter(|r| !r.is_empty())
        .map(|r| (r, env::var("COMMA_PREV_COMMAND").unwrap_or_default()));
    Context {
        shell,
        cwd,
        os,
        nu_available,
        previous,
    }
}

#[cfg(windows)]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(nu: bool, prev: Option<(&str, &str)>) -> Context {
        Context {
            shell: Shell::Bash,
            cwd: "/home/u/proj".into(),
            os: "Fedora Linux 44".into(),
            nu_available: nu,
            previous: prev.map(|(a, b)| (a.into(), b.into())),
        }
    }

    #[test]
    fn user_message_includes_request_cwd_os() {
        let m = user_message("list big files", &ctx(true, None));
        assert!(
            m.contains("list big files")
                && m.contains("/home/u/proj")
                && m.contains("Fedora Linux 44")
        );
    }

    #[test]
    fn user_message_reports_nu_present() {
        assert!(user_message("x", &ctx(true, None)).contains("nushell: available"));
    }

    #[test]
    fn user_message_reports_nu_absent() {
        assert!(user_message("x", &ctx(false, None)).contains("nushell: not installed"));
    }

    #[test]
    fn user_message_includes_previous() {
        let m = user_message(
            "now sort by date",
            &ctx(true, Some(("list big files", "ls -lS"))),
        );
        assert!(m.contains("list big files") && m.contains("ls -lS"));
    }

    #[test]
    fn user_message_omits_previous_section_when_none() {
        assert!(!user_message("x", &ctx(true, None)).contains("Previous"));
    }

    #[test]
    fn system_prompt_pins_format() {
        for s in [
            "\"kind\"",
            "command",
            "answer",
            "warning",
            "single line",
            "nu -c",
            "no markdown",
            "current directory",
            "error suppression",
        ] {
            assert!(BASH_PROMPT.contains(s), "missing {s}");
        }
    }

    #[test]
    fn shell_from_env_parses() {
        assert_eq!(shell_from_env(None), Ok(Shell::Bash));
        assert_eq!(shell_from_env(Some("")), Ok(Shell::Bash));
        assert_eq!(shell_from_env(Some("bash")), Ok(Shell::Bash));
        assert_eq!(shell_from_env(Some("cmd")), Ok(Shell::Cmd));
        assert!(shell_from_env(Some("zsh")).unwrap_err().contains("zsh"));
    }

    #[test]
    fn user_message_marks_cmd_shell() {
        let cmd = Context {
            shell: Shell::Cmd,
            ..ctx(false, None)
        };
        assert!(user_message("x", &cmd).contains("shell: cmd"));
        assert!(!user_message("x", &ctx(false, None)).contains("\nshell:"));
    }

    #[test]
    fn cmd_prompt_pins_format() {
        for s in [
            "\"kind\"",
            "command",
            "answer",
            "warning",
            "single line",
            "cmd.exe",
            "powershell -NoProfile -c",
            "nu -c",
            "no markdown",
            "current directory",
            "error suppression",
        ] {
            assert!(CMD_PROMPT.contains(s), "missing {s}");
        }
    }

    #[test]
    fn bash_prompt_unchanged() {
        assert_eq!(system_prompt(Shell::Bash), BASH_PROMPT);
        assert_eq!(system_prompt(Shell::Cmd), CMD_PROMPT);
    }
}
