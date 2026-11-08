use serde_json::{Value, json};
use std::process::Command;
use std::time::Duration;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-haiku-5-5";
/// Own variable, so Claude Code does not pick the key up from ANTHROPIC_API_KEY.
const KEY_ENV: &str = "COMMA_API_KEY";
const API_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum BackendError {
    NoKey,
    Http(u16, String),
    Network(String),
    Other(String),
}

/// Fall back to `claude -p` only when the API is unusable for a non-network reason.
pub fn should_fallback(e: &BackendError) -> bool {
    matches!(e, BackendError::NoKey | BackendError::Http(..))
}

pub fn request_body(system: &str, user: &str) -> Value {
    json!({
        "model": MODEL,
        "max_tokens": 400,
        // Haiku 5.5 thinks by default; that can use the whole token budget
        // and leave no text, and a one-line reply doesn't need it.
        "thinking": {"type": "disabled"},
        "system": system,
        "messages": [{"role": "user", "content": user}],
    })
}

pub fn extract_text(resp: &Value) -> Result<String, BackendError> {
    resp["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find_map(|b| b["text"].as_str()))
        .map(str::to_string)
        .ok_or_else(|| BackendError::Other("API response had no text".into()))
}

pub fn call_api(key: Option<&str>, system: &str, user: &str) -> Result<String, BackendError> {
    let key = key.filter(|k| !k.is_empty()).ok_or(BackendError::NoKey)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(API_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let mut resp = agent
        .post(API_URL)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .send(request_body(system, user).to_string())
        .map_err(|e| BackendError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        let body = resp.body_mut().read_to_string().unwrap_or_default();
        return Err(BackendError::Http(status, body));
    }
    let v: Value = resp
        .body_mut()
        .read_to_string()
        .ok()
        .and_then(|b| serde_json::from_str(&b).ok())
        .ok_or_else(|| BackendError::Other("bad API response".into()))?;
    extract_text(&v)
}

pub fn call_claude_cli(system: &str, user: &str) -> Result<String, BackendError> {
    let out = Command::new("claude")
        .args([
            "-p",
            "--model",
            "haiku",
            "--tools",
            "",
            "--system-prompt",
            system,
            user,
        ])
        .output()
        .map_err(|e| BackendError::Other(format!("could not run claude: {e}")))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(BackendError::Other(format!(
            "claude failed: {}",
            err.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Ask the API, falling back to `claude -p`. Errors are short one-line strings.
pub fn complete(system: &str, user: &str) -> Result<String, String> {
    let key = std::env::var(KEY_ENV).ok();
    match call_api(key.as_deref(), system, user) {
        Ok(text) => Ok(text),
        Err(e) if should_fallback(&e) => call_claude_cli(system, user).map_err(|f| {
            format!(
                "{} and claude fallback failed: {}",
                describe(&e),
                describe(&f)
            )
        }),
        Err(e) => Err(describe(&e)),
    }
}

fn describe(e: &BackendError) -> String {
    match e {
        BackendError::NoKey => format!("no {KEY_ENV}"),
        BackendError::Http(s, body) => {
            let msg = serde_json::from_str::<Value>(body).ok();
            match msg.as_ref().and_then(|v| v["error"]["message"].as_str()) {
                Some(m) => format!("API returned {s}: {m}"),
                None => format!("API returned {s}"),
            }
        }
        BackendError::Network(m) => format!("network error: {m}"),
        BackendError::Other(m) => m.lines().next().unwrap_or("").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_on_no_key_and_http() {
        assert!(should_fallback(&BackendError::NoKey));
        assert!(should_fallback(&BackendError::Http(401, "".into())));
        assert!(should_fallback(&BackendError::Http(529, "".into())));
    }

    #[test]
    fn no_fallback_on_network() {
        assert!(!should_fallback(&BackendError::Network("timeout".into())));
        assert!(!should_fallback(&BackendError::Other("x".into())));
    }

    #[test]
    fn body_has_model_system_and_user() {
        let b = request_body("SYS", "USER");
        assert_eq!(b["model"], "claude-haiku-5-5");
        assert_eq!(b["system"], "SYS");
        assert_eq!(b["messages"][0]["role"], "user");
        assert_eq!(b["messages"][0]["content"], "USER");
        assert_eq!(b["max_tokens"], 400);
        assert_eq!(b["thinking"]["type"], "disabled");
    }

    #[test]
    fn extracts_first_text_block() {
        let r = serde_json::json!({"content":[{"type":"text","text":"{\"kind\":\"answer\",\"text\":\"hi\"}"}]});
        assert_eq!(
            extract_text(&r).unwrap(),
            "{\"kind\":\"answer\",\"text\":\"hi\"}"
        );
    }

    #[test]
    fn extract_errors_without_text() {
        assert!(extract_text(&serde_json::json!({"content":[]})).is_err());
    }

    #[test]
    fn key_comes_from_comma_api_key() {
        assert_eq!(KEY_ENV, "COMMA_API_KEY");
        assert!(describe(&BackendError::NoKey).contains("COMMA_API_KEY"));
    }

    #[test]
    fn call_api_without_key_is_nokey() {
        assert!(matches!(call_api(None, "s", "u"), Err(BackendError::NoKey)));
    }
}
