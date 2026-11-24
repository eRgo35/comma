# comma

Bash (and cmd.exe + Clink) Enter hook + Rust binary: a line starting with `,` is sent to Claude, and the reply is either a command placed on the prompt line (never auto-run) or a short printed answer. Design: `docs/superpowers/specs/2026-11-04-comma-design.md`. User-facing docs: `README.md`.

## Layout

- `src/hook.bash`: the bash side, embedded via `include_str!` and printed by `comma init bash`. `__comma_process` holds the line logic (testable without readline); `__comma_hook` does the readline rebinding trick.
- `src/hook.lua`: the Clink (cmd.exe) side, printed by `comma init cmd`. Binds Enter; passes everything to `comma ask` through env vars, never on a cmd command line.
- `src/main.rs`: CLI (`init bash|cmd`, `ask [<request>]`). Contract for `ask`: exit 0 = command on stdout, 2 = answer printed to stderr, 1 = error printed to stderr. Only the command goes to stdout. With no argument the request comes from `COMMA_REQUEST`; `COMMA_SHELL` is `bash` (default) or `cmd` and picks the prompt and risk check.
- `src/prompt.rs`: system prompt + per-request context (cwd, OS, nu availability, previous request from `COMMA_PREV_*` env).
- `src/backend.rs`: Anthropic API via ureq (5s timeout, key from `COMMA_API_KEY`), falling back to `claude -p` only on missing key or HTTP error status, never on network failure.
- `src/reply.rs`: parses the model's JSON reply.
- `tests/hook.rs`: `__comma_process` against a stub `comma`. `tests/readline.rs` + `tests/pty_drive.py`: real interactive bash through a pty (needs `python3`). Unix only.
- `tests/clink.rs` + `tests/conpty_drive.py`: real cmd.exe + Clink through a ConPTY (Windows only; needs Clink and `pip install pywinpty`, skipped otherwise).

## Commands

```bash
cargo test                                  # all tests, incl. pty tests (~3s)
cargo clippy --all-targets -- -D warnings   # CI fails on warnings
cargo fmt --check
```

On Windows the same commands also run `tests/clink.rs` (~25s).

## Rules

- Dependencies: only `ureq` and `serde_json`. Ask before adding anything.
- Commits use Conventional Commits (`feat:`, `fix:`, `chore:`, ...). release-please reads them to bump versions and write `CHANGELOG.md`, so the prefix matters. Don't edit the version or CHANGELOG by hand.
- Context sent to the API must never include file names, file contents or shell history (privacy promise in the README).
- The hook must never break normal bash use. Any change to `src/hook.bash` needs a test in `tests/readline.rs` for real readline behaviour (`$_`, heredocs, vi mode, ...).
- Same for cmd: any change to `src/hook.lua` needs a test in `tests/clink.rs`, run on Windows.
- Keep it small: one-shot help, no conversation beyond the previous request, no config file.
