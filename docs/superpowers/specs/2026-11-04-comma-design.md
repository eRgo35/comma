# comma — design

Claude, one keystroke away in bash. For quick one-shot help (build a command, answer a short question), never for heavy work like coding.

## Usage

- Type a line starting with `,` (column 0) and press Enter: `,ls -la where size > 1mb`.
- An Enter hook (`bind -x`) reads the raw line before bash parses it, so `>`, `|`, globs and quotes are passed through untouched.
- Lines without the `,` prefix behave exactly as before. No hotkey (user runs zellij).

## What comes back

The model decides which kind of reply fits:

- **Command** — replaces the prompt line. The user reviews/edits it and presses Enter to run it. Never auto-executed. May be plain bash, or `nu -c '…'` when structured filtering suits nushell better (only if `nu` is on PATH).
- **Risky command** (deletes, overwrites, sudo, network, git push) — a one-line `⚠ <reason>` is printed above the prompt. Still only runs on Enter.
- **Answer** — 1–5 lines of plain text (no markdown) printed below; the prompt line is cleared.
- **Follow-up** — remembers only the last request and the command it produced (per shell session), so `,now sort by date` works.

## History

Both the `,…` request and the command actually run go into bash history.

## Backend

- Direct Anthropic Messages API, model hardcoded to `claude-haiku-5-5`, key from `ANTHROPIC_API_KEY`. No config file.
- Fall back to `claude -p` only when there is no key or the API returns an HTTP error status (401, 429, 5xx, …). No fallback on network failure or timeout.

## Context sent

cwd, OS, whether `nu` is available, the request, and the previous request + produced command. No file names, no command history.

## Failure

Any failure: the line stays exactly as typed, a one-line error is printed, the shell is never blocked. API timeout ~5s; Ctrl-C cancels. If the binary is missing, the `.bashrc` eval is a no-op.

## Build

- One Rust binary `comma`; deps `ureq` + `serde_json` (approved).
- `comma init bash` prints the bash hook.
- "thinking…" indicator on stderr while waiting. Answers are not streamed.
- Install: `cargo install --path .`, then `eval "$(comma init bash)"` in `~/.bashrc`.
