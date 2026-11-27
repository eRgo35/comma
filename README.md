# comma

Ask Claude for a shell command right at your bash prompt, or in cmd.exe with Clink on Windows. Type `,` followed by what you want, press Enter, and the command appears on your prompt line for you to check and run.

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

Requires bash 5.1 or newer on Linux. For Windows, see [Windows (cmd + Clink)](#windows-cmd--clink).

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

## Windows (cmd + Clink)

Requires [Clink](https://github.com/chrisant996/clink) 1.2.46 or newer. Download `comma-<version>-x86_64-pc-windows-msvc.zip` from the [latest release](https://github.com/eRgo35/comma/releases/latest) and put `comma.exe` on your `PATH`.

Then create `%LOCALAPPDATA%\clink\comma.lua` (Clink's scripts folder; `clink info` shows yours) with:

```lua
-- Run comma.exe from PATH by full path: cmd would also look in the current directory.
for dir in (os.getenv("PATH") or ""):gmatch("[^;]+") do
    local exe = path.join((dir:gsub('"', "")), "comma.exe")
    if os.isfile(exe) then
        local f = io.popen('2>nul "' .. exe .. '" init cmd')
        local s = f and f:read("*a") or ""
        if f then f:close() end
        if s ~= "" then load(s, "comma")() end
        break
    end
end
```

Open a new cmd and try `,list files here`.

```console
C:\proj> ,find .log files bigger than 1mb
C:\proj> powershell -NoProfile -c "Get-ChildItem -Recurse -Filter *.log | Where-Object Length -gt 1MB"
```

- Commands are plain cmd when that works, `powershell -NoProfile -c "..."` when cmd can't do it cleanly, and `nu -c '...'` if nushell is installed.
- The API key is optional. Without it comma uses `claude -p`, which it finds on `PATH` as `claude.exe` (the native installer or Scoop). An npm install of Claude Code (`claude.cmd`) is not found.
- If you do set a key (`setx COMMA_API_KEY sk-ant-...`), note that `setx` puts it in the environment of every program you start (Explorer, editors, PowerShell). The hook clears it inside each cmd, so programs started from that cmd don't get it; only comma does. After a Clink script reload (Ctrl-X Ctrl-R), or in a cmd started from that cmd, the key is gone for that session and comma falls back to `claude -p`.
- `,` lines go into Clink history.
- Works in emacs and vi editing modes. `( ... More?` continuation lines are never touched.

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

A rate limit, overload or server error is retried once after a short pause. If no key is set, or the API still returns an error (bad key, rate limit, outage), comma falls back to the [Claude Code](https://claude.com/claude-code) CLI (`claude -p`) if you have it. The fallback works with your Claude subscription but takes a few seconds per request, and gives up after 30 seconds. It runs without your MCP servers, settings, hooks and plugins, so the model gets no tools and none of their context, and it doesn't save a session. Claude Code still adds some context of its own to each request, such as your account email.

## When something fails

If there is no network, the request times out, or anything else goes wrong, you get a one-line error and your line stays as you typed it. Your shell never depends on comma: if the binary is missing, the `.bashrc` line above does nothing.

Ctrl-C cancels a request. Bash discards the typed line on Ctrl-C, but the request is already in history, so press ↑ to get it back.

## Privacy

Each request sends:

- the text after `,`
- your current directory path
- your OS name
- which shell (only when it's cmd)
- whether `nu` is installed
- your previous request and the command it produced

comma never sends file names, file contents or your shell history. Anything you type after `,`, including secrets, goes to the API.

## Name clash

[nix-community/comma](https://github.com/nix-community/comma) also uses `,` as a command. If you have it installed, the Enter hook takes precedence for lines starting with `,`.

## License

[MIT](LICENSE)
