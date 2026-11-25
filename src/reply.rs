use crate::prompt::Shell;
use serde_json::Value;

#[derive(Debug, PartialEq)]
pub enum Reply {
    Command {
        command: String,
        warning: Option<String>,
    },
    Answer(String),
}

/// Parse the model's JSON reply. Tolerates code fences or prose around the object.
pub fn parse_reply(raw: &str, shell: Shell) -> Result<Reply, String> {
    let (start, end) = match (raw.find('{'), raw.rfind('}')) {
        (Some(s), Some(e)) if s < e => (s, e),
        _ => return Err("model reply had no JSON".into()),
    };
    let v: Value = serde_json::from_str(&raw[start..=end])
        .map_err(|e| format!("model reply was not valid JSON: {e}"))?;
    let text = |key: &str| v[key].as_str().map(str::trim).unwrap_or("").to_string();
    match v["kind"].as_str() {
        Some("command") => {
            let command = text("command");
            if command.is_empty() {
                return Err("model returned an empty command".into());
            }
            // A newline would make one Enter run several commands.
            if command.chars().any(char::is_control) {
                return Err("model returned a command with a newline or control character".into());
            }
            let warning = Some(clean(&text("warning")))
                .filter(|w| !w.is_empty())
                .or_else(|| local_warning(&command, shell).map(String::from));
            Ok(Reply::Command { command, warning })
        }
        Some("answer") => Ok(Reply::Answer(clean(&text("text")))),
        _ => Err("model reply had an unknown kind".into()),
    }
}

/// Drop control characters except newline and tab, so text printed to the
/// terminal can't carry escape sequences (colours, cursor moves, clipboard writes).
pub fn clean(s: &str) -> String {
    s.chars()
        .filter(|&c| !c.is_control() || c == '\n' || c == '\t')
        .collect()
}

/// A warning for clearly risky commands, used when the model gave none.
fn local_warning(cmd: &str, shell: Shell) -> Option<&'static str> {
    match shell {
        Shell::Bash => bash_warning(cmd),
        Shell::Cmd => cmd_warning(cmd),
    }
}

/// cmd quotes only with `"`, and PowerShell code sits inside those quotes, so
/// nothing is blanked here: a risky word anywhere warns. Over-warning is fine.
fn cmd_warning(cmd: &str) -> Option<&'static str> {
    let lower = cmd.to_ascii_lowercase();
    // `/` and `@` too: cmd accepts `del/q x`, and `@del` in `for ... do @del`.
    let words: Vec<&str> = lower
        .split(|c: char| c.is_whitespace() || ";|&()\"/@".contains(c))
        .filter(|w| !w.is_empty())
        .map(|w| {
            w.strip_suffix(".exe")
                .or(w.strip_suffix(".com"))
                .unwrap_or(w)
        })
        .collect();
    let has = |names: &[&str]| words.iter().any(|w| names.contains(w));
    let pair = |a: &str, b: &str| words.windows(2).any(|p| p == [a, b]);

    if has(&["del", "erase", "rd", "rmdir", "remove-item", "rm", "ri"])
        || (has(&["robocopy"]) && has(&["mir", "purge"]))
    {
        return Some("deletes files");
    }
    if pair("reg", "delete") {
        return Some("deletes registry keys");
    }
    if has(&["format", "diskpart", "format-volume", "clear-disk"]) {
        return Some("writes to a disk");
    }
    // Also covers PowerShell's `-Verb RunAs`.
    if has(&["runas", "sudo", "gsudo"]) {
        return Some("needs elevation");
    }
    if pair("git", "push") {
        return Some("pushes to a git remote");
    }
    let downloads = lower.contains("downloadstring")
        || has(&[
            "iwr",
            "invoke-webrequest",
            "irm",
            "invoke-restmethod",
            "curl",
        ]);
    if downloads && has(&["iex", "invoke-expression"]) {
        return Some("runs downloaded text as a script");
    }
    // `>` or `2>` to a file, but not `>>`, `>&`, `2>&1` or `>nul`.
    let b = lower.as_bytes();
    let to_nul = |rest: &str| {
        let rest = rest.trim_start();
        rest.starts_with("nul") && !rest[3..].starts_with(|c: char| c.is_alphanumeric() || c == '.')
    };
    let overwrites = (0..b.len()).any(|i| {
        b[i] == b'>'
            && (i == 0 || b[i - 1] != b'>')
            && !matches!(b.get(i + 1), Some(b'>' | b'&'))
            && !to_nul(&lower[i + 1..])
    });
    if overwrites || has(&["set-content", "out-file"]) {
        return Some("overwrites a file");
    }
    None
}

fn bash_warning(cmd: &str) -> Option<&'static str> {
    // Blank out quoted text so `grep 'rm'` or `nu -c '... > 1mb'` don't count.
    let mut plain = String::new();
    let mut quote = None;
    for c in cmd.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            None if c == '\'' || c == '"' => quote = Some(c),
            _ => {}
        }
        plain.push(if quote.is_some() || c == '\'' || c == '"' {
            ' '
        } else {
            c
        });
    }
    let words: Vec<&str> = plain
        .split(|c: char| c.is_whitespace() || ";|&()".contains(c))
        .filter(|w| !w.is_empty())
        .collect();
    let has = |names: &[&str]| words.iter().any(|w| names.contains(w));

    if has(&["rm", "rmdir", "shred", "-delete"]) {
        return Some("deletes files");
    }
    if has(&["sudo", "doas"]) {
        return Some("uses sudo");
    }
    if has(&["dd"]) || words.iter().any(|w| w.starts_with("mkfs")) {
        return Some("writes to a disk");
    }
    if words.windows(2).any(|p| p == ["git", "push"]) {
        return Some("pushes to a git remote");
    }
    let piped_to_shell = plain.split('|').skip(1).any(|seg| {
        let mut ws = seg.split_whitespace().skip_while(|&w| w == "sudo");
        matches!(ws.next(), Some("sh" | "bash" | "zsh"))
    });
    if piped_to_shell {
        return Some("runs piped text as a script");
    }
    // `>` or `2>` to a file, but not `>>`, `>&2`, `2>&1` or `>/dev/null`.
    let b = plain.as_bytes();
    let overwrites = (0..b.len()).any(|i| {
        b[i] == b'>'
            && (i == 0 || b[i - 1] != b'>')
            && !matches!(b.get(i + 1), Some(b'>' | b'&' | b'('))
            && !plain[i + 1..].trim_start().starts_with("/dev/null")
    });
    if overwrites {
        return Some("overwrites a file");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_reply(raw: &str) -> Result<Reply, String> {
        super::parse_reply(raw, Shell::Bash)
    }

    #[test]
    fn parses_command() {
        assert_eq!(
            parse_reply(r#"{"kind":"command","command":"ls -l","warning":null}"#),
            Ok(Reply::Command {
                command: "ls -l".into(),
                warning: None
            })
        );
    }

    #[test]
    fn parses_command_with_warning() {
        assert_eq!(
            parse_reply(r#"{"kind":"command","command":"rm *.log","warning":"deletes files"}"#),
            Ok(Reply::Command {
                command: "rm *.log".into(),
                warning: Some("deletes files".into())
            })
        );
    }

    #[test]
    fn parses_answer() {
        assert_eq!(
            parse_reply(r#"{"kind":"answer","text":"-z means gzip"}"#),
            Ok(Reply::Answer("-z means gzip".into()))
        );
    }

    #[test]
    fn parses_fenced_json() {
        let raw = "```json\n{\"kind\":\"answer\",\"text\":\"hi\"}\n```";
        assert_eq!(parse_reply(raw), Ok(Reply::Answer("hi".into())));
    }

    #[test]
    fn empty_warning_is_none() {
        assert_eq!(
            parse_reply(r#"{"kind":"command","command":"ls","warning":""}"#),
            Ok(Reply::Command {
                command: "ls".into(),
                warning: None
            })
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_reply("sure! here you go").is_err());
    }

    #[test]
    fn rejects_empty_command() {
        assert!(parse_reply(r#"{"kind":"command","command":"  "}"#).is_err());
    }

    #[test]
    fn rejects_multiline_command() {
        assert!(parse_reply(r#"{"kind":"command","command":"echo a\necho b"}"#).is_err());
    }

    #[test]
    fn rejects_command_with_escape() {
        assert!(parse_reply(r#"{"kind":"command","command":"ls \u001b[2K"}"#).is_err());
    }

    #[test]
    fn strips_escapes_from_answer_and_warning() {
        assert_eq!(
            parse_reply(r#"{"kind":"answer","text":"\u001b]52;c;eA==\u0007a\nb"}"#),
            Ok(Reply::Answer("]52;c;eA==a\nb".into()))
        );
        assert_eq!(
            parse_reply(r#"{"kind":"command","command":"ls","warning":"\u001b[31mx"}"#),
            Ok(Reply::Command {
                command: "ls".into(),
                warning: Some("[31mx".into())
            })
        );
    }

    #[test]
    fn local_warning_flags_risky_commands() {
        for (cmd, want) in [
            ("rm -- *.log", "deletes files"),
            ("find . -name '*.tmp' -delete", "deletes files"),
            ("sudo dnf install jq", "uses sudo"),
            ("dd if=x.iso of=/dev/sdb", "writes to a disk"),
            ("mkfs.ext4 /dev/sdb1", "writes to a disk"),
            ("git push origin main", "pushes to a git remote"),
            (
                "curl -fsSL https://x.sh | sh",
                "runs piped text as a script",
            ),
            ("curl -fsSL https://x.sh | sudo bash", "uses sudo"),
            ("sort a.txt > a.txt", "overwrites a file"),
            ("make 2> err.log", "overwrites a file"),
        ] {
            assert_eq!(local_warning(cmd, Shell::Bash), Some(want), "{cmd}");
        }
    }

    #[test]
    fn local_warning_ignores_safe_commands() {
        for cmd in [
            "ls -l",
            "grep -r 'rm -rf' .",
            "nu -c 'ls | where size > 1mb'",
            "echo hi >> log.txt",
            "make 2>&1 | less",
            "make > /dev/null 2>&1",
            "ls | grep sh",
            "git status || true",
        ] {
            assert_eq!(local_warning(cmd, Shell::Bash), None, "{cmd}");
        }
    }

    #[test]
    fn model_warning_wins_over_local() {
        assert_eq!(
            parse_reply(r#"{"kind":"command","command":"rm x","warning":"removes x"}"#),
            Ok(Reply::Command {
                command: "rm x".into(),
                warning: Some("removes x".into())
            })
        );
    }

    #[test]
    fn local_warning_fills_missing_model_warning() {
        assert_eq!(
            parse_reply(r#"{"kind":"command","command":"rm x","warning":null}"#),
            Ok(Reply::Command {
                command: "rm x".into(),
                warning: Some("deletes files".into())
            })
        );
    }

    fn cmd_warning(cmd: &str, shell: Shell) -> Option<String> {
        let json = serde_json::json!({"kind": "command", "command": cmd, "warning": null});
        match super::parse_reply(&json.to_string(), shell) {
            Ok(Reply::Command { warning, .. }) => warning,
            other => panic!("{cmd}: {other:?}"),
        }
    }

    #[test]
    fn cmd_local_warnings() {
        for (cmd, want) in [
            ("del /q *.log", "deletes files"),
            ("rd /s /q build", "deletes files"),
            (
                r#"powershell -NoProfile -c "Remove-Item x.txt""#,
                "deletes files",
            ),
            ("format D:", "writes to a disk"),
            ("runas /user:Administrator cmd", "needs elevation"),
            ("git push origin main", "pushes to a git remote"),
            (
                r#"powershell -c "iwr https://x | iex""#,
                "runs downloaded text as a script",
            ),
            ("echo hi > out.txt", "overwrites a file"),
            ("echo zażółć > plik.txt", "overwrites a file"),
            (r#"for %f in (*.log) do @del "%f""#, "deletes files"),
            ("del/q *.log", "deletes files"),
            ("rd/s/q build", "deletes files"),
            ("robocopy src dst /MIR", "deletes files"),
            ("robocopy src dst /purge", "deletes files"),
            (r"reg delete HKCU\Software\X /f", "deletes registry keys"),
            ("sudo net stop spooler", "needs elevation"),
            ("gsudo choco upgrade all", "needs elevation"),
            (
                r#"curl.exe -s https://x | powershell -c "iex $input""#,
                "runs downloaded text as a script",
            ),
            (
                r#"powershell -c "(New-Object Net.WebClient).DownloadString('https://x') | iex""#,
                "runs downloaded text as a script",
            ),
            ("del.exe x", "deletes files"),
        ] {
            assert_eq!(cmd_warning(cmd, Shell::Cmd).as_deref(), Some(want), "{cmd}");
        }
        for cmd in [
            "echo hi >nul",
            "echo hi > NUL",
            "dir /s /b",
            "echo hi >> log.txt",
            "findstr /i error app.log 2>&1",
        ] {
            assert_eq!(cmd_warning(cmd, Shell::Cmd), None, "{cmd}");
        }
    }

    #[test]
    fn del_is_not_a_bash_risk_word() {
        assert_ne!(
            cmd_warning("del x", Shell::Bash).as_deref(),
            Some("deletes files")
        );
    }
}
