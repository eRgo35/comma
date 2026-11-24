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
