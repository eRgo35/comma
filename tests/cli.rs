use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_comma"))
}

#[test]
fn init_bash_prints_hook() {
    let o = bin().args(["init", "bash"]).output().unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).contains("__comma_hook"));
}

#[test]
fn unknown_args_exit_1() {
    assert_eq!(bin().arg("nope").output().unwrap().status.code(), Some(1));
}

#[test]
fn init_cmd_prints_hook() {
    let o = bin().args(["init", "cmd"]).output().unwrap();
    assert!(o.status.success());
    assert!(!o.stdout.is_empty());
}

#[test]
fn ask_without_request_errors() {
    let o = bin()
        .arg("ask")
        .env_remove("COMMA_REQUEST")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("no request"));
}

/// No key and no `claude` on PATH, so the backend fails without touching the
/// network; reaching that error shows the request came from COMMA_REQUEST.
#[test]
fn ask_reads_request_from_env() {
    let empty = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("empty-path");
    std::fs::create_dir_all(&empty).unwrap();
    let o = bin()
        .arg("ask")
        .env("COMMA_REQUEST", "hi")
        .env_remove("COMMA_API_KEY")
        .env("PATH", &empty)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&o.stderr).contains("no COMMA_API_KEY"),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn ask_rejects_unknown_shell() {
    let o = bin()
        .args(["ask", "x"])
        .env("COMMA_SHELL", "zsh")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("unknown COMMA_SHELL"));
}

/// The `claude -p` fallback must not hand the model the user's MCP servers,
/// plugin hooks or settings, nor save a session per request.
#[cfg(unix)]
#[test]
fn claude_fallback_runs_isolated() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("claude-stub");
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("claude");
    std::fs::write(
        &stub,
        "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$0.args\"\n\
         printf '%s' '{\"kind\":\"command\",\"command\":\"ls\",\"warning\":null}'\n",
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let o = bin()
        .args(["ask", "-rf list files"])
        .env_remove("COMMA_API_KEY")
        .env_remove("COMMA_SHELL")
        .env("PATH", &dir)
        .output()
        .unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(o.stdout, b"ls");
    let raw = std::fs::read(dir.join("claude.args")).unwrap();
    let args: Vec<&str> = std::str::from_utf8(&raw)
        .unwrap()
        .strip_suffix('\0')
        .unwrap()
        .split('\0')
        .collect();
    let has_pair = |a: &str, b: &str| args.windows(2).any(|w| w == [a, b]);
    assert!(has_pair("--tools", ""), "{args:?}");
    assert!(has_pair("--setting-sources", ""), "{args:?}");
    assert!(args.contains(&"--strict-mcp-config"), "{args:?}");
    assert!(!args.contains(&"--mcp-config"), "{args:?}");
    assert!(args.contains(&"--no-session-persistence"), "{args:?}");
    // The request is the last argument, after "--", so "-rf" isn't read as a flag.
    assert_eq!(args[args.len() - 2], "--", "{args:?}");
    assert!(
        args[args.len() - 1].ends_with("Request: -rf list files"),
        "{args:?}"
    );
}
