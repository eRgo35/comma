#![cfg(windows)]
//! End-to-end tests of the Clink hook in a real cmd.exe (through a ConPTY via
//! pywinpty), with a stub `comma.cmd` that returns `%STUB_OUT%` as the command.
//! Generated commands are `set /a` sums so their output (70007) differs from
//! the text on the line (`set /a 70000+7`): "on the line" vs "ran" is visible.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CLINK_DEFAULT: &str = r"C:\Program Files (x86)\clink\clink_x64.exe";
const GEN: &str = "set /a 70000+7";
const GEN_RAN: &str = "70007";

/// Logs with `set NAME`, which prints the value without cmd parsing it.
const STUB: &str = "@echo off\r\n\
>>\"%STUB_LOG%\" set COMMA_REQUEST\r\n\
>>\"%STUB_LOG%\" set COMMA_PREV\r\n\
>>\"%STUB_LOG%\" set COMMA_SHELL\r\n\
>\"%STUB_LOG%.key\" set COMMA_API_KEY\r\n\
if \"%STUB_RC%\"==\"2\" (>&2 echo STUB-ANSWER& exit /b 2)\r\n\
if \"%STUB_RC%\"==\"1\" (>&2 echo STUB-ERROR& exit /b 1)\r\n\
echo %STUB_OUT%\r\n\
exit /b 0\r\n";

struct Opts {
    rc: u8,
    vi: bool,
    stub: bool,
    /// Another Clink script, loaded before comma.lua (scripts load by name).
    other_script: Option<&'static str>,
    /// A `comma.cmd` in the current directory, which must never be run.
    planted: bool,
}

const DEFAULT: Opts = Opts {
    rc: 0,
    vi: false,
    stub: true,
    other_script: None,
    planted: false,
};

const PLANTED: &str =
    "@echo off\r\n>\"%STUB_LOG%.planted\" echo PLANTED\r\necho echo PLANTED-RAN\r\n";

struct Run {
    /// Output of each chunk, in order.
    chunks: Vec<String>,
    log: String,
    /// `COMMA_API_KEY=...` as the stub last saw it ("" if it never ran).
    key: String,
    cwd: PathBuf,
}

fn clink_exe() -> String {
    std::env::var("CLINK_EXE").unwrap_or_else(|_| CLINK_DEFAULT.into())
}

fn available() -> bool {
    let winpty = Command::new("python")
        .args(["-I", "-c", "import winpty"])
        .output()
        .is_ok_and(|o| o.status.success());
    let clink = Path::new(&clink_exe()).is_file();
    if !winpty || !clink {
        eprintln!(
            "skipping: needs Clink ({}) and python with pywinpty",
            clink_exe()
        );
    }
    winpty && clink
}

fn drive(name: &str, o: Opts, keys: &[&str]) -> Option<Run> {
    if !available() {
        return None;
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("clink-{name}"));
    let _ = fs::remove_dir_all(&dir);
    let (profile, bin, cwd) = (dir.join("profile"), dir.join("bin"), dir.join("cwd"));
    for d in [&profile, &bin, &cwd] {
        fs::create_dir_all(d).unwrap();
    }
    let hook = Command::new(env!("CARGO_BIN_EXE_comma"))
        .args(["init", "cmd"])
        .output()
        .unwrap();
    fs::write(profile.join("comma.lua"), &hook.stdout).unwrap();
    // Loads after comma.lua; the driver sends no keys until it sees this.
    fs::write(profile.join("zzz_ready.lua"), "print('<<<READY>>>')\n").unwrap();
    if o.vi {
        fs::write(profile.join("clink_inputrc"), "set editing-mode vi\n").unwrap();
    }
    if let Some(script) = o.other_script {
        fs::write(profile.join("aaa_other.lua"), script).unwrap();
    }
    if o.planted {
        fs::write(cwd.join("comma.cmd"), PLANTED).unwrap();
    }
    fs::write(bin.join("comma.cmd"), STUB).unwrap();
    let log = dir.join("log");
    fs::write(&log, "").unwrap();

    let path = std::env::var("PATH").unwrap();
    let path = if o.stub {
        format!("{};{path}", bin.display())
    } else {
        path
    };
    let out = Command::new("python")
        .arg("-I")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/conpty_drive.py"
        ))
        .arg(&profile)
        .args(keys)
        .current_dir(&cwd)
        .env("PATH", path)
        .env("CLINK_EXE", clink_exe())
        .env("STUB_LOG", &log)
        .env("STUB_OUT", GEN)
        .env("STUB_RC", o.rc.to_string())
        .env("COMMA_API_KEY", "test-key")
        .output()
        .unwrap();
    let screen = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "driver failed ({}):\n{}\n{screen}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let chunks = (1..=keys.len())
        .map(|i| {
            let start = screen
                .find(&format!("<<<CHUNK {i}>>>"))
                .unwrap_or_else(|| panic!("no chunk {i}:\n{screen}"));
            let rest = &screen[start..];
            let end = rest[1..].find("<<<CHUNK ").map_or(rest.len(), |e| e + 1);
            rest[..end].to_string()
        })
        .collect();
    Some(Run {
        chunks,
        log: fs::read_to_string(&log).unwrap(),
        key: fs::read_to_string(dir.join("log.key")).unwrap_or_default(),
        cwd,
    })
}

#[test]
fn clink_normal_line_runs() {
    let Some(r) = drive("normal", DEFAULT, &["set /a 40000+2\r"]) else {
        return;
    };
    assert!(r.chunks[0].contains("40002"), "{}", r.chunks[0]);
    assert_eq!(r.log, "");
}

#[test]
fn clink_comma_line_puts_command_without_running() {
    let Some(r) = drive("basic", DEFAULT, &[",say hi", "\r", "\r"]) else {
        return;
    };
    assert!(r.chunks[1].contains(GEN), "{}", r.chunks[1]);
    assert!(!r.chunks[1].contains(GEN_RAN), "ran early: {}", r.chunks[1]);
    assert!(r.chunks[2].contains(GEN_RAN), "{}", r.chunks[2]);
    assert!(r.log.contains("COMMA_REQUEST=say hi\r\n"), "{}", r.log);
    assert!(r.log.contains("COMMA_SHELL=cmd"), "{}", r.log);
}

#[test]
fn clink_special_chars_arrive_intact() {
    let req = r#"a "b" & c > d %PATH% !x!"#;
    let line = format!(",{req}");
    let Some(r) = drive("special", DEFAULT, &[&line, "\r"]) else {
        return;
    };
    assert!(
        r.log.contains(&format!("COMMA_REQUEST={req}\r\n")),
        "{}",
        r.log
    );
    let all = r.chunks.concat();
    assert!(!all.contains("is not recognized"), "{all}");
    assert!(!r.cwd.join("d").exists());
}

#[test]
fn clink_answer_clears_line() {
    let o = Opts { rc: 2, ..DEFAULT };
    let Some(r) = drive("answer", o, &[",what", "\r", "set /a 40000+2\r"]) else {
        return;
    };
    assert!(r.chunks[1].contains("STUB-ANSWER"), "{}", r.chunks[1]);
    assert!(r.chunks[2].contains("40002"), "{}", r.chunks[2]);
    assert!(
        !r.chunks[2].contains("is not recognized"),
        "{}",
        r.chunks[2]
    );
}

#[test]
fn clink_error_leaves_line() {
    let o = Opts { rc: 1, ..DEFAULT };
    let Some(r) = drive("error", o, &[",oops", "\r", "\x1b", "set /a 40000+2\r"]) else {
        return;
    };
    assert!(r.chunks[1].contains("STUB-ERROR"), "{}", r.chunks[1]);
    assert!(
        r.chunks[1].contains(",oops"),
        "line not kept: {}",
        r.chunks[1]
    );
    assert!(r.chunks[3].contains("40002"), "{}", r.chunks[3]);
    assert_eq!(r.log.matches("COMMA_REQUEST=").count(), 1, "{}", r.log);
}

#[test]
fn clink_key_hidden_from_children() {
    let ps = r#"powershell -NoProfile -c "'K=['+$env:COMMA_API_KEY+']'""#;
    let Some(r) = drive(
        "key",
        DEFAULT,
        &[",go", "\r", "\r", &format!("{ps}\r\x1fK=[]")],
    ) else {
        return;
    };
    assert!(r.key.contains("COMMA_API_KEY=test-key"), "{}", r.key);
    assert!(r.chunks[3].contains("K=[]"), "{}", r.chunks[3]);
}

#[test]
fn clink_previous_request_passed() {
    let Some(r) = drive("prev", DEFAULT, &[",first", "\r", "\r", ",second", "\r"]) else {
        return;
    };
    assert!(r.log.contains("COMMA_PREV_REQUEST=first\r\n"), "{}", r.log);
    assert!(
        r.log.contains(&format!("COMMA_PREV_COMMAND={GEN}\r\n")),
        "{}",
        r.log
    );
}

#[test]
fn clink_request_added_to_history() {
    let keys = [r#",hist "me" & you"#, "\r", "\r", "history 5\r"];
    let Some(r) = drive("history", DEFAULT, &keys) else {
        return;
    };
    // Quotes and all, as typed.
    assert!(
        r.chunks[3].contains(r#",hist "me" & you"#),
        "{}",
        r.chunks[3]
    );
    assert!(r.chunks[3].contains(GEN), "{}", r.chunks[3]);
}

#[test]
fn clink_vi_mode() {
    let o = Opts {
        vi: true,
        ..DEFAULT
    };
    let Some(r) = drive("vi", o, &[",say hi", "\r", "\r"]) else {
        return;
    };
    assert!(r.chunks[1].contains(GEN), "{}", r.chunks[1]);
    assert!(!r.chunks[1].contains(GEN_RAN), "ran early: {}", r.chunks[1]);
    assert!(r.chunks[2].contains(GEN_RAN), "{}", r.chunks[2]);
}

#[test]
fn clink_continuation_not_intercepted() {
    let Some(r) = drive("cont", DEFAULT, &["(\r", ",x\r", ")\r"]) else {
        return;
    };
    assert_eq!(r.log, "");
}

#[test]
fn clink_blank_request_is_ignored() {
    let Some(r) = drive("blank", DEFAULT, &[",   ", "\r", "set /a 40000+2\r"]) else {
        return;
    };
    assert_eq!(r.log, "");
    assert!(r.chunks[2].contains("40002"), "{}", r.chunks[2]);
    assert!(
        !r.chunks[2].contains("is not recognized"),
        "{}",
        r.chunks[2]
    );
}

/// Ctrl-X Ctrl-R reloads the scripts (Clink resets key bindings first, so the
/// hook's guard against saving its own binding isn't reachable this way).
#[test]
fn clink_works_after_script_reload() {
    let keys = ["\x18\x12", "set /a 40000+2\r", ",go", "\r", "\r"];
    let Some(r) = drive("reload", DEFAULT, &keys) else {
        return;
    };
    assert!(r.chunks[1].contains("40002"), "{}", r.chunks[1]);
    assert!(r.chunks[4].contains(GEN_RAN), "{}", r.chunks[4]);
    // Accepted limitation: a reload finds the key already cleared.
    assert!(r.key.contains("COMMA_API_KEY="), "{}", r.key);
}

#[test]
fn clink_comma_missing_leaves_line_and_enter_still_works() {
    let o = Opts {
        stub: false,
        ..DEFAULT
    };
    let Some(r) = drive("missing", o, &[",x", "\r", "\x1b", "set /a 40000+2\r"]) else {
        return;
    };
    assert!(r.chunks[3].contains("40002"), "{}", r.chunks[3]);
    let all = r.chunks.concat();
    assert!(
        !all.contains("'x' is not recognized"),
        "cmd ran the line: {all}"
    );
}

/// Ctrl-J is accept-line too; without the hook cmd would run `,del x` as `del x`.
/// A plain "\n" doesn't reach Clink as Ctrl-J through ConPTY, so send the key
/// as a win32-input-mode event (VK_J, Ctrl held), down then up.
const CTRL_J: &str = "\x1b[74;36;10;1;8;1_\x1b[74;36;10;0;8;1_";

#[test]
fn clink_ctrl_j_goes_through_hook() {
    let Some(r) = drive("ctrlj", DEFAULT, &[",say hi", CTRL_J, "\r"]) else {
        return;
    };
    assert!(r.log.contains("COMMA_REQUEST=say hi\r\n"), "{}", r.log);
    assert!(!r.chunks[1].contains(GEN_RAN), "ran early: {}", r.chunks[1]);
    assert!(r.chunks[2].contains(GEN_RAN), "{}", r.chunks[2]);
}

/// Another script bound Enter to its own luafunc before comma loaded.
#[test]
fn clink_enter_works_with_other_script_binding() {
    let other = "function other_enter(rl_buffer) rl.invokecommand(\"accept-line\") end\n\
                 rl.setbinding([[\"\\r\"]], [[\"luafunc:other_enter\"]], \"emacs\")\n";
    let o = Opts {
        other_script: Some(other),
        ..DEFAULT
    };
    let Some(r) = drive("other", o, &["set /a 40000+2\r", ",go", "\r", "\r"]) else {
        return;
    };
    assert!(r.chunks[0].contains("40002"), "{}", r.chunks[0]);
    assert!(r.chunks[3].contains(GEN_RAN), "{}", r.chunks[3]);
}

/// cmd looks in the current directory before PATH; the hook must run the
/// comma found on PATH, never a comma.cmd that happens to be in cwd.
#[test]
fn clink_ignores_comma_in_current_dir() {
    let o = Opts {
        planted: true,
        ..DEFAULT
    };
    let Some(r) = drive("planted", o, &[",go", "\r"]) else {
        return;
    };
    assert!(r.log.contains("COMMA_REQUEST=go"), "{}", r.log);
    assert!(!r.cwd.parent().unwrap().join("log.planted").exists());
    assert!(r.chunks[1].contains(GEN), "{}", r.chunks[1]);
}
