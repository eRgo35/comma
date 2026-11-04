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
            let warning = Some(text("warning")).filter(|w| !w.is_empty());
            Ok(Reply::Command { command, warning })
        }
        Some("answer") => Ok(Reply::Answer(text("text"))),
        _ => Err("model reply had an unknown kind".into()),
    }
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
}
