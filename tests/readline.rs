//! End-to-end tests of the Enter binding in a real interactive bash (via a pty),
//! with a stub `comma` that returns `$STUB_OUT` as the generated command.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

const STUB: &str =
    "#!/bin/bash\nprintf '%s\\n' \"$2\" >> \"$STUB_LOG\"\nprintf '%s' \"$STUB_OUT\"\n";

struct Run {
    screen: String,
    log: String,
}

fn drive(name: &str, stub_out: &str, keys: &[&str]) -> Run {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("readline-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let hook = Command::new(env!("CARGO_BIN_EXE_comma"))
        .args(["init", "bash"])
        .output()
        .unwrap();
    fs::write(dir.join("hook.bash"), &hook.stdout).unwrap();
    let stub = dir.join("comma");
    fs::write(&stub, STUB).unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    let log = dir.join("log");
    fs::write(&log, "").unwrap();

    let o = Command::new("python3")
        .arg("-I")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/pty_drive.py"))
        .arg(dir.join("hook.bash"))
        .args(keys)
        .env(
            "PATH",
            format!("{}:{}", dir.display(), std::env::var("PATH").unwrap()),
        )
        .env("STUB_LOG", &log)
        .env("STUB_OUT", stub_out)
        .output()
        .unwrap();
    Run {
        screen: String::from_utf8_lossy(&o.stdout).into(),
        log: fs::read_to_string(&log).unwrap(),
    }
}

#[test]
fn comma_line_then_enter_runs_generated_command() {
    let r = drive("basic", "echo GEN-$((40+2))", &[",say hi\r", "\r"]);
    assert!(r.screen.contains("GEN-42"), "{}", r.screen);
    assert_eq!(r.log, "say hi\n");
}

#[test]
fn dollar_underscore_preserved() {
    let r = drive("underscore", "", &["echo abc\r", "echo \"[$_]\"\r"]);
    assert!(r.screen.contains("[abc]"), "{}", r.screen);
}

#[test]
fn heredoc_line_not_intercepted() {
    let r = drive(
        "heredoc",
        "SHOULD-NOT-RUN",
        &["cat <<EOF\r", ",a,b\r", "EOF\r"],
    );
    assert!(r.log.is_empty(), "hook fired inside heredoc: {}", r.log);
    assert!(!r.screen.contains("SHOULD-NOT-RUN"), "{}", r.screen);
}

#[test]
fn vi_insert_mode_uses_hook() {
    let r = drive(
        "vi-insert",
        "echo VI-$((1+1))",
        &["set -o vi\r", ",x\r", "\r"],
    );
    assert!(r.screen.contains("VI-2"), "{}", r.screen);
    assert!(!r.screen.contains("command not found"), "{}", r.screen);
}

#[test]
fn vi_command_mode_uses_hook() {
    let r = drive(
        "vi-command",
        "echo VC-$((2+2))",
        &["set -o vi\r", ",x", "\x1b", "\r", "\r"],
    );
    assert!(r.screen.contains("VC-4"), "{}", r.screen);
    assert!(!r.screen.contains("command not found"), "{}", r.screen);
}
