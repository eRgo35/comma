use serde_json::{Value, json};
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-haiku-5-5";
/// Own variable, so Claude Code does not pick the key up from ANTHROPIC_API_KEY.
const KEY_ENV: &str = "COMMA_API_KEY";
const API_TIMEOUT: Duration = Duration::from_secs(5);
const CLI_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_DELAY: Duration = Duration::from_millis(300);

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

/// Rate limits, overload and server errors are often gone a moment later, and
/// one more API call is much faster than starting `claude -p`.
pub fn should_retry(e: &BackendError) -> bool {
    matches!(e, BackendError::Http(s, _) if *s == 429 || *s >= 500)
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
        // Constrain the reply to the two shapes the system prompt asks for,
        // so it is always one valid JSON object.
        "output_config": {"format": {"type": "json_schema", "schema": reply_schema()}},
    })
}

fn reply_schema() -> Value {
    json!({
        "anyOf": [
            {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "const": "command"},
                    "command": {"type": "string"},
                    "warning": {"type": ["string", "null"]},
                },
                "required": ["kind", "command", "warning"],
                "additionalProperties": false,
            },
            {
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "const": "answer"},
                    "text": {"type": "string"},
                },
                "required": ["kind", "text"],
                "additionalProperties": false,
            },
        ]
    })
}

pub fn extract_text(resp: &Value) -> Result<String, BackendError> {
    match resp["stop_reason"].as_str() {
        Some("max_tokens") => {
            return Err(BackendError::Other(
                "reply was cut off at the token limit".into(),
            ));
        }
        Some("refusal") => return Err(BackendError::Other("model refused the request".into())),
        _ => {}
    }
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
    let mut cmd = Command::new("claude");
    cmd.args([
        "-p",
        "--model",
        "haiku",
        // `--tools ""` only removes built-in tools. Also leave out the user's
        // MCP servers (mail, files, desktop control, ...), and their settings,
        // hooks and plugins, which add context and tools this one-line reply
        // must not have. Don't save a session per request either.
        "--tools",
        "",
        "--strict-mcp-config",
        "--setting-sources",
        "",
        "--no-session-persistence",
        "--system-prompt",
        system,
        // So a request starting with "-" isn't read as a flag.
        "--",
        user,
    ]);
    let out = run_with_timeout(cmd, CLI_TIMEOUT)?;
    if !out.status.success() {
        return Err(BackendError::Other(format!(
            "claude failed: {}",
            failure_line(&out)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `claude -p` prints its error on stdout and notices (sign-in expiry, ...) on
/// stderr, so prefer the last line of stdout, then the last line of stderr.
fn failure_line(out: &Output) -> String {
    let last = |b: &[u8]| {
        String::from_utf8_lossy(b)
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(String::from)
    };
    last(&out.stdout)
        .or_else(|| last(&out.stderr))
        .unwrap_or_else(|| format!("exit status {}", out.status))
}

/// Read `pipe` to the end on its own thread and send `(index, bytes)`.
fn read_in_background<R: Read + Send + 'static>(
    pipe: Option<R>,
    index: usize,
    tx: mpsc::Sender<(usize, Vec<u8>)>,
) {
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf);
        }
        let _ = tx.send((index, buf));
    });
}

/// Run `cmd` capturing its output, killing it after `limit`.
fn run_with_timeout(mut cmd: Command, limit: Duration) -> Result<Output, BackendError> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BackendError::Other(format!("could not run claude: {e}")))?;
    let deadline = Instant::now() + limit;
    let timed_out = |child: &mut std::process::Child| {
        let _ = child.kill();
        let _ = child.wait();
        BackendError::Other(format!("claude timed out after {}s", limit.as_secs()))
    };
    // Read both pipes while the child runs: a child whose output fills a pipe
    // buffer blocks until it is read, and would never exit.
    let (tx, rx) = mpsc::channel();
    read_in_background(child.stdout.take(), 0, tx.clone());
    read_in_background(child.stderr.take(), 1, tx);
    let mut bufs = [Vec::new(), Vec::new()];
    for _ in 0..2 {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok((i, buf)) => bufs[i] = buf,
            Err(_) => return Err(timed_out(&mut child)),
        }
    }
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => return Err(timed_out(&mut child)),
            Err(e) => return Err(BackendError::Other(format!("could not run claude: {e}"))),
        }
    };
    let [stdout, stderr] = bufs;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Ask the API, falling back to `claude -p`. Errors are short one-line strings.
pub fn complete(system: &str, user: &str) -> Result<String, String> {
    let key = std::env::var(KEY_ENV).ok();
    let mut result = call_api(key.as_deref(), system, user);
    if result.as_ref().is_err_and(should_retry) {
        thread::sleep(RETRY_DELAY);
        result = call_api(key.as_deref(), system, user);
    }
    match result {
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
    fn retry_on_rate_limit_and_server_errors_only() {
        for s in [429, 500, 529] {
            assert!(should_retry(&BackendError::Http(s, "".into())), "{s}");
        }
        for s in [400, 401, 403, 404] {
            assert!(!should_retry(&BackendError::Http(s, "".into())), "{s}");
        }
        assert!(!should_retry(&BackendError::NoKey));
        assert!(!should_retry(&BackendError::Network("timeout".into())));
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
    fn body_constrains_reply_to_both_kinds() {
        let f = &request_body("SYS", "USER")["output_config"]["format"];
        assert_eq!(f["type"], "json_schema");
        let kinds: Vec<&str> = f["schema"]["anyOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["properties"]["kind"]["const"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["command", "answer"]);
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
    fn extract_reports_truncation_and_refusal() {
        let cut = serde_json::json!({"stop_reason":"max_tokens","content":[{"type":"text","text":"{\"kind\""}]});
        let refused = serde_json::json!({"stop_reason":"refusal","content":[]});
        for (r, want) in [(cut, "cut off"), (refused, "refused")] {
            match extract_text(&r) {
                Err(BackendError::Other(m)) => assert!(m.contains(want), "{m}"),
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn run_with_timeout_kills_slow_command() {
        let mut cmd = Command::new("sleep");
        cmd.arg("5");
        let start = Instant::now();
        assert!(run_with_timeout(cmd, Duration::from_millis(200)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn run_with_timeout_returns_output() {
        let (mut cmd, want): (Command, &[u8]) = if cfg!(windows) {
            (Command::new("cmd"), b"hi\r\n")
        } else {
            (Command::new("echo"), b"hi\n")
        };
        cmd.args(if cfg!(windows) {
            &["/c", "echo hi"][..]
        } else {
            &["hi"][..]
        });
        let out = run_with_timeout(cmd, Duration::from_secs(5)).unwrap();
        assert_eq!(out.stdout, want);
    }

    /// More output than a pipe buffer holds (64 KiB on Linux, 4 KiB on
    /// Windows) on stdout and stderr, which must be read while the child runs.
    #[test]
    fn run_with_timeout_reads_large_output() {
        let file = std::env::temp_dir().join(format!("comma-big-{}.txt", std::process::id()));
        let big = "0123456789abcdef".repeat(20_000);
        std::fs::write(&file, &big).unwrap();
        let f = file.display().to_string();
        for to_stderr in [false, true] {
            #[cfg(windows)]
            let cmd = {
                // raw_arg: Rust would escape the quotes as \", which cmd doesn't read.
                use std::os::windows::process::CommandExt;
                let mut cmd = Command::new("cmd");
                let redirect = if to_stderr { " 1>&2" } else { "" };
                cmd.arg("/c").raw_arg(format!("type \"{f}\"{redirect}"));
                cmd
            };
            #[cfg(unix)]
            let cmd = {
                let mut cmd = Command::new("sh");
                let redirect = if to_stderr { " >&2" } else { "" };
                cmd.args(["-c", &format!("cat \"$0\"{redirect}"), &f]);
                cmd
            };
            let out = run_with_timeout(cmd, Duration::from_secs(5)).unwrap();
            let got = if to_stderr { &out.stderr } else { &out.stdout };
            assert_eq!(got.len(), big.len(), "stderr: {to_stderr}");
            assert!(out.status.success());
        }
        let _ = std::fs::remove_file(&file);
    }

    /// The child exits but a process it left behind keeps the pipe open:
    /// still give up at the limit instead of waiting for it.
    #[cfg(unix)]
    #[test]
    fn run_with_timeout_limit_holds_with_leftover_process() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 5 & echo hi"]);
        let start = Instant::now();
        assert!(run_with_timeout(cmd, Duration::from_millis(300)).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn failure_line_prefers_stdout_then_last_stderr_line() {
        #[cfg(unix)]
        let status = std::os::unix::process::ExitStatusExt::from_raw(1 << 8);
        #[cfg(windows)]
        let status = std::os::windows::process::ExitStatusExt::from_raw(1);
        let out = |o: &str, e: &str| Output {
            status,
            stdout: o.into(),
            stderr: e.into(),
        };
        assert_eq!(
            failure_line(&out("bad model\n", "Warning: sign-in ends\n")),
            "bad model"
        );
        assert_eq!(
            failure_line(&out("", "Warning: sign-in ends\nnot logged in\n\n")),
            "not logged in"
        );
        assert!(failure_line(&out("", "")).contains("exit status"));
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
