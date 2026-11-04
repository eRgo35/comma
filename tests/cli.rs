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
