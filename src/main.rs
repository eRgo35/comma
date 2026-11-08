mod backend;
mod prompt;
mod reply;

use reply::Reply;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;

const HOOK: &str = include_str!("hook.bash");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["init", "bash"] => {
            print!("{HOOK}");
            ExitCode::SUCCESS
        }
        ["ask", request] => ask(request),
        _ => {
            eprintln!("usage: comma init bash | comma ask <request>");
            ExitCode::from(1)
        }
    }
}

/// Exit 0: command on stdout. Exit 2: answer printed to stderr. Exit 1: error.
fn ask(request: &str) -> ExitCode {
    let ctx = prompt::gather_context();
    let msg = prompt::user_message(request, &ctx);

    let spinner = std::io::stderr().is_terminal();
    if spinner {
        eprint!("thinking…");
        let _ = std::io::stderr().flush();
    }
    let result =
        backend::complete(prompt::SYSTEM_PROMPT, &msg).and_then(|raw| reply::parse_reply(&raw));
    if spinner {
        eprint!("\r\x1b[2K");
    }

    match result {
        Ok(Reply::Command { command, warning }) => {
            if let Some(w) = warning {
                eprintln!("⚠ {w}");
            }
            print!("{command}");
            ExitCode::SUCCESS
        }
        Ok(Reply::Answer(text)) => {
            eprintln!("{text}");
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("comma: {}", reply::clean(&e));
            ExitCode::from(1)
        }
    }
}
