# comma for cmd.exe + Clink: design

## Why

The author uses cmd.exe with Clink (in Windows Terminal) every day and wants the same `,request` + Enter flow there as in bash. The goal is a native cmd experience for that setup, not broad Windows coverage. PowerShell as an interactive shell is out of scope.

## Goals

- In cmd with Clink, a line starting with `,` (column 0) + Enter sends the request to comma. A command lands on the input line, never run. An answer is printed and the line is cleared. Errors print one line and leave the line as typed. This is the same contract as bash.
- Commands are plain cmd by default. When cmd can't do it cleanly, the command is `powershell -NoProfile -c "..."`. `nu -c` stays as an option when `nu` is installed.
- `COMMA_API_KEY` is kept out of programs started from cmd, as in bash.
- Refining the previous request (`,now only .txt`) works within one cmd session.
- comma builds, tests and ships for `x86_64-pc-windows-msvc`.

## Non-goals and anti-goals

- No PowerShell hook, no plain-cmd support without Clink, no Git Bash tools assumed on PATH.
- No config file, no conversation beyond the previous request (CLAUDE.md "keep it small").
- Anti-goal: the hook must never break normal cmd use. If comma is missing, the Lua errors, or the request fails, Enter on a normal line behaves as before. A `,` line must never run as a cmd command. Without the hook, cmd treats `,` as a delimiter and would run `list ...` for `,list ...`.
- Anti-goal: never send file names, file contents or history (README privacy promise).

## Constraints (verified in a spike on Windows 11, Clink 1.9.37, Lua 5.2)

- Binding Enter: `rl.getbinding([["\r"]], keymap)` returns the original (`accept-line` by default). `rl.setbinding([["\r"]], [["luafunc:comma_enter"]], keymap)` replaces it. For non-`,` lines the function calls `rl.invokecommand(original)`.
- Inside the luafunc: `rl_buffer:getbuffer()`, `beginoutput()` (so printed output doesn't overwrite the line), `remove(1, len+1)`, `insert(text)`, `refreshline()`.
- `io.popen(cmdline)` runs through `cmd /c`. **The request must not be on the command line**: `&`, `>`, `%VAR%`, `!x!`, `"` get interpreted. Passing it through an env var (`os.setenv("COMMA_REQUEST", ...)` before `io.popen("comma ask")`) arrived intact.
- `f:close()` returns `ok, "exit", code`, so the exit code is available. stderr from the child goes straight to the console, so answers, warnings and the spinner show.
- Env clearing: `os.setenv(name, nil)` does **not** clear it as cmd's children see it. `os.setenv(name, "")` does: children see an empty value. comma already treats an empty key as missing.
- `( ... More?` continuation lines are read by cmd itself, not Clink, so the hook never sees them. No equivalent of bash's `__COMMA_PRIMARY` is needed.
- `"<CLINK_EXE>" --session <clink.getsession()> history add <line>` adds to the current session's history. Run via `cmd /d /v:on /c "... history add !COMMA_HISTLINE!"` so the line comes from an env var. The quotes in the line are lost (`"a"` becomes `a`).
- Overhead of the two popens: about 0.15s.
- Real Clink can be driven non-interactively through a ConPTY console (SSH provides one, and pywinpty does too).

## Approaches considered

Hook mechanism:

1. **Lua function bound to Enter (chosen).** It reads and replaces the line before cmd sees it, mirroring the bash `bind -x` trick.
2. `clink.onfilterinput`: rejected. It runs after the line is accepted, so the suggested command would run immediately.
3. doskey macro `,=...`: rejected. It can't put text on the input line.

Command style: plain cmd + PowerShell fallback + nu (chosen) vs. plain cmd only (too many weak answers) vs. assume Git Bash tools on PATH (not on cmd's PATH by default, and `find` clashes).

Hook tests: real Clink in a ConPTY driven by Python with **pywinpty** (chosen by the user) vs. hand-written ConPTY via ctypes (more code) vs. Lua stubs on Linux (doesn't test real Clink).

## Design

### Binary (`src/`)

- `comma init cmd` prints the Lua hook (`src/hook.lua`, embedded via `include_str!`).
- `comma ask` with **no** argument reads the request from `COMMA_REQUEST`. `comma ask <request>` is unchanged. Exit codes are unchanged (0 command on stdout, 2 answer on stderr, 1 error).
- `COMMA_SHELL` selects the target shell: unset or `bash` gives today's behaviour, `cmd` gives the cmd variant. The hook sets it. Any other value is an error (exit 1).
- `prompt.rs`:
  - Shell-specific system prompt. The bash prompt stays word for word, and the existing tests keep passing.
  - The cmd prompt asks for a single line valid in cmd.exe and prefers plain cmd builtins and standard Windows tools. It uses `powershell -NoProfile -c "..."` only when cmd can't do it cleanly, and `nu -c` only when nushell is available and clearly simpler. It sets the same warning rule and the same answer format.
  - The context adds `shell: cmd`. Nothing else new is sent.
  - nu detection works on Windows: look for `nu.exe` on PATH (`cfg(windows)`: the file exists; `cfg(unix)`: as today).
  - OS: on Windows, `/etc/os-release` is absent, so it falls back to `env::consts::OS` ("windows"). Good enough.
- `reply.rs`: `local_warning` takes the shell. The bash list is unchanged. For cmd it matches case-insensitive words with no quote blanking, since cmd only quotes with `"` and PowerShell code sits inside quotes, so blanking would hide it. Over-warning is acceptable.
  - deletes: `del`, `erase`, `rd`, `rmdir`, `Remove-Item`, `rm`, `ri`
  - disk: `format`, `diskpart`, `Format-Volume`, `Clear-Disk`
  - elevation: `runas`, `-Verb RunAs`
  - git: `git push`
  - script from network: `iex` / `Invoke-Expression` together with `iwr` / `Invoke-WebRequest` / `irm` / `Invoke-RestMethod`
  - overwrites: `Set-Content`, `Out-File`, plus a single `>` that isn't `>>`, `>&`, `2>&1`, `>nul` or `>NUL`
- `backend.rs`: the test that uses `std::os::unix::process::ExitStatusExt` gets a `cfg(unix)` gate (or a portable status). The `claude -p` fallback stays `Command::new("claude")`. The author installs Claude Code with Scoop (into a non-standard Scoop dir). Scoop's Main-bucket `claude-code` package ships the native `claude.exe`, and Scoop puts a `claude.exe` shim in `<scoop>\shims`, so it is found through PATH wherever Scoop lives. The native installer's `claude.exe` is found the same way. An npm `claude.cmd` is not found, which is acceptable and noted in the README. Things to verify on the VM: arguments pass through the Scoop shim intact (the multi-line system prompt and a `--` request), claude's error text reaches comma's one-line error, and the 30s timeout kill also ends the real `claude.exe` behind the shim, not just the shim.
- Spinner `\r\x1b[2K` works in Windows Terminal. Unchanged.

### Hook (`src/hook.lua`)

At load: read `COMMA_API_KEY` into a local, then `os.setenv("COMMA_API_KEY", "")`. Bind Enter in the `emacs`, `vi-insert` and `vi-command` keymaps, remembering each keymap's original binding. Don't bind anything if `comma` isn't found on PATH. The loader below already guards that case.

`comma_enter(rl_buffer)`:

1. If the line doesn't start with `,`, invoke the original binding and return.
2. If the text after `,` is all whitespace, clear the line and return, as bash does.
3. `rl_buffer:beginoutput()`.
4. Set `COMMA_REQUEST`, `COMMA_SHELL=cmd`, `COMMA_PREV_REQUEST`, `COMMA_PREV_COMMAND`, and `COMMA_API_KEY` = the saved key.
5. Run `io.popen("comma ask")`, read stdout, then `close()` to get the code.
6. Immediately set `COMMA_API_KEY`, `COMMA_REQUEST`, `COMMA_SHELL` and the prev vars back to `""`.
7. Add the `,` line to history via `clink history add` (constraint above).
8. Exit code 0: replace the line with the output and remember the prev request and command. Code 2: clear the line, prev request = request, prev command = "". Anything else: leave the line as typed.
9. `refreshline()`.

Any Lua error in steps 3–9 must not leave the key set or the line accepted. Wrap the body in `pcall`, then always run step 6.

### Install (README)

Create `%LOCALAPPDATA%\clink\comma.lua` containing a loader. This is the cmd equivalent of `eval "$(comma init cmd)"`, so the hook never goes stale after an upgrade:

```lua
local f = io.popen("comma init cmd 2>nul")
if f then local s = f:read("*a"); f:close(); if s ~= "" then load(s, "comma")() end end
```

The key is optional. The author uses no key on Windows, so `claude -p` is the main path there and must work. A user who sets one (`setx COMMA_API_KEY sk-ant-...`) gets it cleared inside each cmd session by the hook, so only comma gets it. Known limitation, documented and not fixed: after a Clink script reload, or in a cmd started from a hooked cmd, the hook sees the cleared value and comma falls back to `claude -p`. The user chose this over a registry fallback. README gets a "Windows (cmd + Clink)" install section, a short cmd example, a note that `,` request history loses quotes, and the key limitation.

Reloading the hook must not capture its own `luafunc:comma_enter` as the "original" binding (that would make Enter recurse). If the current binding is already `luafunc:comma_enter`, keep the previously saved original, or use `accept-line`.

### Tests

- Existing bash tests (`tests/hook.rs`, `tests/readline.rs`) get `#![cfg(unix)]`.
- Unit tests:
  - the cmd system prompt pins its key phrases
  - `user_message` includes `shell: cmd` only for cmd
  - cmd `local_warning` cases (`del /q *.log`, `rd /s /q x`, `powershell -NoProfile -c "Remove-Item x"`, `echo hi > out.txt` warn; `dir /s`, `echo hi >nul` don't)
  - `COMMA_SHELL` parsing
  - `comma ask` reading `COMMA_REQUEST` (CLI test with no network: no key, `claude` not on PATH, so expect exit 1 and an error that mentions no key, not a usage error)
- `tests/clink.rs` (`#![cfg(windows)]`) and `tests/conpty_drive.py`: like `readline.rs`, start `cmd /d /k` with `clink inject --quiet --profile <tmpdir>` through pywinpty, with a stub `comma.cmd` first on PATH. The stub logs `COMMA_REQUEST` and the key it saw, and returns `STUB_OUT`/`STUB_RC`. Cases:
  - a normal line runs untouched
  - a `,` line puts the command on the line without running it, and a second Enter runs it
  - a request containing `& > " %PATH% !x!` arrives intact and nothing runs
  - an answer (rc 2) clears the line
  - an error (rc 1) leaves the line
  - a program started from cmd sees an empty key, and the stub sees the real key
  - the previous request is passed on the next `,` line
  - `,` is added to history
  - vi mode works
  - `( ... More? ,x` is not intercepted
  - Clink and pywinpty missing: the tests are skipped with a message (like the python3 check).
- CI: a `windows-latest` job runs fmt, clippy (`-D warnings`) and `cargo test`, after installing Clink (signed setup from the GitHub release, `/S`) and `pip install pywinpty`.
- Release: add an `x86_64-pc-windows-msvc` build on `windows-latest`, packaged as `comma-<ver>-x86_64-pc-windows-msvc.zip` with `comma.exe`, LICENSE, README and a `.sha256`.
- Development testing happens on the local Windows VM (`ssh win`, source synced via `~/Windows/shared`).

### Docs

- README: Windows section, as above.
- CLAUDE.md layout: `src/hook.lua`, `tests/clink.rs` and the Windows commands. Rule: any change to `src/hook.lua` needs a test in `tests/clink.rs`.
- The old design doc stays as is.

## Left to the builder

- The exact cmd system prompt wording, within the rules above.
- The pywinpty driver details (timeouts, ANSI stripping).
- How CI caches the Clink and pywinpty installs.
