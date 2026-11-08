# comma

Ask Claude for a shell command right at your bash prompt. Type `,` followed by what you want, press Enter, and the command appears on your prompt line for you to check and run.

```console
$ ,list files bigger than 1mb here
$ nu -c 'ls | where type == file and size > 1mb | select name size'   ← put on your line, not run yet
                                                                       ← press Enter to run it
$ ,now sort them by date
$ nu -c 'ls | where type == file and size > 1mb | sort-by modified'

$ ,delete all .log files
⚠ deletes files
$ rm -- *.log

$ ,what does tar -z do
-z filters the archive through gzip: compresses when creating, decompresses when extracting.
$
```

comma is for quick one-shot help: building a command or answering a short question. It is not an agent and never runs anything by itself.

## Install

Requires bash 5.1 or newer on Linux.

Download a binary from the [latest release](https://github.com/eRgo35/comma/releases/latest) (x86_64 and aarch64, static) and put `comma` on your `PATH`, or build it:

```bash
cargo install --git https://github.com/eRgo35/comma
```

Then add to `~/.bashrc`:

```bash
export COMMA_API_KEY="sk-ant-..."   # or: "$(cat ~/path/to/key)"
command -v comma >/dev/null && eval "$(comma init bash)"
```

Open a new shell and try `,list files here`.

The hook un-exports `COMMA_API_KEY` and passes it only to comma, so other programs you start from that shell (build tools, `npm`/`pnpm` scripts, ...) can't read it.

## How it works

- Only lines that **start** with `,` at the main prompt are touched. Everything else, including heredoc and continuation lines, behaves exactly as before.
- Your text is passed to comma raw, before bash parses it, so `>`, `|`, quotes and globs are safe to type.
- Claude replies with either a command, which replaces your line, or a short answer, which is printed. Risky commands (deleting, overwriting, sudo, network, git push) get a `⚠` line. comma also checks the command itself for the clearly risky ones, in case Claude leaves the warning out. Commands that span several lines or contain control characters are rejected.
- comma remembers your last request in the current shell, so you can refine it: `,now only .rs files`.
- Both your `,…` request and the command you run go into bash history.
- When nushell (`nu`) is installed and structured filtering is easier, comma may suggest `nu -c '…'`.
- Works in emacs and vi editing modes.

## Backend

comma calls the Anthropic API directly with `claude-haiku-5-5` and a 5 second timeout, using `COMMA_API_KEY`. It deliberately does not read `ANTHROPIC_API_KEY`, so exporting the key does not change how Claude Code bills you.

If no key is set, or the API returns an error (bad key, rate limit, outage), comma falls back to the [Claude Code](https://claude.com/claude-code) CLI (`claude -p`) if you have it. The fallback works with your Claude subscription but takes several seconds per request, and gives up after 30 seconds.

## When something fails

If there is no network, the request times out, or anything else goes wrong, you get a one-line error and your line stays as you typed it. Your shell never depends on comma: if the binary is missing, the `.bashrc` line above does nothing.

Ctrl-C cancels a request. Bash discards the typed line on Ctrl-C, but the request is already in history, so press ↑ to get it back.

## Privacy

Each request sends:

- the text after `,`
- your current directory path
- your OS name
- whether `nu` is installed
- your previous request and the command it produced

comma never sends file names, file contents or your shell history. Anything you type after `,`, including secrets, goes to the API.

## Name clash

[nix-community/comma](https://github.com/nix-community/comma) also uses `,` as a command. If you have it installed, the Enter hook takes precedence for lines starting with `,`.

## License

[MIT](LICENSE)
