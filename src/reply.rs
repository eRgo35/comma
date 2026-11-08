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
pub fn parse_reply(raw: &str) -> Result<Reply, String> {
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
                .or_else(|| local_warning(&command).map(String::from));
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
fn local_warning(cmd: &str) -> Option<&'static str> {
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
            assert_eq!(local_warning(cmd), Some(want), "{cmd}");
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
            assert_eq!(local_warning(cmd), None, "{cmd}");
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
}
