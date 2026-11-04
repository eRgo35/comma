use std::env;
use std::path::Path;

pub struct Context {
    pub cwd: String,
    pub os: String,
    pub nu_available: bool,
    /// (previous request, command it produced; "" if it was an answer)
    pub previous: Option<(String, String)>,
}

pub const SYSTEM_PROMPT: &str = r#"You are a bash assistant inside the user's terminal. The user types a short request; you reply with ONLY one JSON object and nothing else.

If the user wants something done or listed, reply:
{"kind":"command","command":"<command>","warning":null}
- The command must be a single line that is valid in bash.
- Use `nu -c '...'` only when nushell is available and structured filtering is clearly simpler that way; otherwise use plain bash and coreutils.
- Set "warning" to a short reason (max 5 words) if the command deletes or overwrites files, uses sudo, touches the network, or pushes to a git remote. Otherwise null.

If the user asks a question, reply:
{"kind":"answer","text":"<answer>"}
- The answer is 1-5 lines of plain text, no markdown.

If a previous request is given, the new request may refine it."#;

pub fn user_message(request: &str, ctx: &Context) -> String {
    let nu = if ctx.nu_available { "available" } else { "not installed" };
    let mut m = format!("cwd: {}\nos: {}\nnushell: {nu}\n", ctx.cwd, ctx.os);
    if let Some((req, cmd)) = &ctx.previous {
        m.push_str(&format!("Previous request: {req}\nPrevious command: {cmd}\n"));
    }
    m.push_str(&format!("Request: {request}"));
    m
}

pub fn gather_context() -> Context {
    let cwd = env::current_dir().map(|p| p.display().to_string()).unwrap_or_default();
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string()))
        })
        .unwrap_or_else(|| env::consts::OS.to_string());
    let nu_available = env::var_os("PATH")
        .map(|p| env::split_paths(&p).any(|d| is_executable(&d.join("nu"))))
        .unwrap_or(false);
    let previous = env::var("COMMA_PREV_REQUEST")
        .ok()
        .filter(|r| !r.is_empty())
        .map(|r| (r, env::var("COMMA_PREV_COMMAND").unwrap_or_default()));
    Context { cwd, os, nu_available, previous }
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(nu: bool, prev: Option<(&str, &str)>) -> Context {
        Context {
            cwd: "/home/u/proj".into(),
            os: "Fedora Linux 44".into(),
            nu_available: nu,
            previous: prev.map(|(a, b)| (a.into(), b.into())),
        }
    }

    #[test]
    fn user_message_includes_request_cwd_os() {
        let m = user_message("list big files", &ctx(true, None));
        assert!(m.contains("list big files") && m.contains("/home/u/proj") && m.contains("Fedora Linux 44"));
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
        let m = user_message("now sort by date", &ctx(true, Some(("list big files", "ls -lS"))));
        assert!(m.contains("list big files") && m.contains("ls -lS"));
    }

    #[test]
    fn user_message_omits_previous_section_when_none() {
        assert!(!user_message("x", &ctx(true, None)).contains("Previous"));
    }

    #[test]
    fn system_prompt_pins_format() {
        for s in ["\"kind\"", "command", "answer", "warning", "single line", "nu -c", "no markdown"] {
            assert!(SYSTEM_PROMPT.contains(s), "missing {s}");
        }
    }
}
