//! Tests the hook's line logic (`__comma_process`) against a stub `comma`,
//! without readline. `p LINE` runs one Enter press and prints the resulting
//! READLINE_LINE and __COMMA_HANDLED on two lines.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const STUB: &str = r#"#!/bin/bash
printf '%s\t%s|%s\n' "$2" "$COMMA_PREV_REQUEST" "$COMMA_PREV_COMMAND" >> "$STUB_LOG"
printf '%s' "$STUB_OUT"
exit "$STUB_RC"
"#;

struct Run {
    out: Vec<String>,
    log: Vec<String>,
}

fn tmpdir() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "hook-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&d).unwrap();
    d
}

/// `lines` are passed as env vars L1, L2, ...; `script` uses them as `p "$L1"`.
fn run(lines: &[&str], rc: i32, stub_out: &str, script: &str) -> Run {
    let dir = tmpdir();
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

    let full = format!(
        "set -o history; source \"$HOOK\"\n\
         p() {{ READLINE_LINE=$1; __comma_process; printf '%s\\n%s\\n' \"$READLINE_LINE\" \"$__COMMA_HANDLED\"; }}\n{script}"
    );
    let mut cmd = Command::new("bash");
    cmd.args(["--norc", "--noprofile", "-c", &full])
        .env("HOOK", dir.join("hook.bash"))
        .env(
            "PATH",
            format!("{}:{}", dir.display(), std::env::var("PATH").unwrap()),
        )
        .env("STUB_LOG", &log)
        .env("STUB_RC", rc.to_string())
        .env("STUB_OUT", stub_out);
    for (i, l) in lines.iter().enumerate() {
        cmd.env(format!("L{}", i + 1), l);
    }
    let o = cmd.output().unwrap();
    assert!(
        o.status.success(),
        "bash failed: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    Run {
        out: String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(String::from)
            .collect(),
        log: fs::read_to_string(&log)
            .unwrap()
            .lines()
            .map(String::from)
            .collect(),
    }
}

#[test]
fn command_replaces_line() {
    let r = run(&[",find big files"], 0, "ls -lS", r#"p "$L1""#);
    assert_eq!(r.out, ["ls -lS", "1"]);
}

#[test]
fn raw_text_passed_verbatim() {
    let line = r#",echo '$HOME' "a b" `x` > f | y"#;
    let r = run(&[line], 0, "true", r#"p "$L1""#);
    assert_eq!(r.log.len(), 1);
    assert_eq!(r.log[0].split('\t').next().unwrap(), &line[1..]);
}

#[test]
fn answer_clears_line() {
    let r = run(&[",what is tar"], 2, "", r#"p "$L1""#);
    assert_eq!(r.out, ["", "1"]);
}

#[test]
fn error_keeps_line_and_blocks_accept() {
    let r = run(&[",find big files"], 1, "", r#"p "$L1""#);
    assert_eq!(r.out, [",find big files", "1"]);
}

#[test]
fn normal_line_untouched() {
    let r = run(&["ls -la"], 0, "SHOULD NOT APPEAR", r#"p "$L1""#);
    assert_eq!(r.out, ["ls -la", "0"]);
    assert!(r.log.is_empty());
}

#[test]
fn empty_request_does_nothing() {
    let r = run(&[",", ",   "], 0, "x", r#"p "$L1"; p "$L2""#);
    assert_eq!(r.out, [",", "1", ",   ", "1"]);
    assert!(r.log.is_empty());
}

#[test]
fn previous_passed_on_next_call() {
    let r = run(
        &[",find big files", ",now sort by date"],
        0,
        "ls -lS",
        r#"p "$L1"; p "$L2""#,
    );
    assert_eq!(r.log[0], "find big files\t|");
    assert_eq!(r.log[1], "now sort by date\tfind big files|ls -lS");
}

#[test]
fn request_added_to_history() {
    let r = run(&[",find big files"], 0, "ls -lS", r#"p "$L1"; history 1"#);
    assert!(r.out.last().unwrap().contains(",find big files"));
}
