-- comma: Clink hook for cmd.exe, printed by `comma init cmd`. Load it from a
-- script in Clink's scripts directory (see README).
-- A line starting with "," is sent to `comma ask` instead of being run.

-- Keep the key in this cmd but out of programs started from it: it's set again
-- only while comma runs. Cleared with "", not nil: a variable removed with nil
-- is still seen by cmd's children.
local key = os.getenv("COMMA_API_KEY") or ""
os.setenv("COMMA_API_KEY", "")

local VARS = {
    "COMMA_API_KEY", "COMMA_REQUEST", "COMMA_SHELL",
    "COMMA_PREV_REQUEST", "COMMA_PREV_COMMAND", "COMMA_HISTLINE",
}
local prev_request, prev_command = "", ""
local original = {} -- original[keymap][key]
local ENTER_KEYS = { [["\r"]], [["\n"]] } -- Enter and Ctrl-J

-- Full path of comma from PATH. `cmd /c comma` would also look in the current
-- directory first, so a comma.bat in a cloned repo would run, with the key.
local function find_comma()
    local exts = os.getenv("PATHEXT") or ".COM;.EXE;.BAT;.CMD"
    for dir in (os.getenv("PATH") or ""):gmatch("[^;]+") do
        dir = dir:gsub('"', "")
        for ext in exts:gmatch("[^;]+") do
            local p = path.join(dir, "comma" .. ext)
            if os.isfile(p) then
                return p
            end
        end
    end
end

-- Everything reaches comma through the environment: io.popen runs its command
-- line through cmd, which would act on "&", ">", "%VAR%" in the request.
local function ask(line)
    local exe = find_comma()
    if not exe then
        error("comma not found on PATH", 0)
    end
    os.setenv("COMMA_REQUEST", line:sub(2))
    os.setenv("COMMA_SHELL", "cmd")
    os.setenv("COMMA_PREV_REQUEST", prev_request)
    os.setenv("COMMA_PREV_COMMAND", prev_command)
    os.setenv("COMMA_API_KEY", key)
    -- Leading "<nul" so the line doesn't start with a quote, which cmd /c strips.
    local f = io.popen('<nul "' .. exe .. '" ask')
    local out = f:read("*a")
    local _, _, code = f:close()
    return out, code
end

-- Delayed expansion (!VAR!) happens after cmd parses the line, so the text is
-- passed as-is (Clink's argument parsing still drops its quotes).
local function add_history(line)
    os.setenv("COMMA_HISTLINE", line)
    local h = io.popen('2>&1 cmd /d /v:on /c ""' .. CLINK_EXE .. '" --session '
        .. clink.getsession() .. ' history add !COMMA_HISTLINE!"')
    if h then
        h:read("*a")
        h:close()
    end
    os.setenv("COMMA_HISTLINE", "")
end

local function set_line(rl_buffer, text)
    rl_buffer:beginundogroup()
    rl_buffer:remove(1, rl_buffer:getlength() + 1)
    rl_buffer:insert(text)
    rl_buffer:endundogroup()
end

local function comma_enter(rl_buffer, key)
    local line = rl_buffer:getbuffer()
    if line:sub(1, 1) ~= "," then
        local mode = rl.getvariable("editing-mode") == "vi" and "vi-insert" or "emacs"
        rl.invokecommand(original[mode][key] or "accept-line")
        return
    end
    if line:sub(2):match("^%s*$") then
        set_line(rl_buffer, "")
        return
    end
    rl_buffer:beginoutput()
    local ok, out, code = pcall(ask, line)
    for _, v in ipairs(VARS) do
        os.setenv(v, "")
    end
    pcall(add_history, line)
    if not ok then
        print("comma: " .. tostring(out))
    elseif code == 0 then
        out = out:gsub("[\r\n]+$", "")
        set_line(rl_buffer, out)
        prev_request, prev_command = line:sub(2), out
    elseif code == 2 then
        set_line(rl_buffer, "")
        prev_request, prev_command = line:sub(2), ""
    end -- any other code: error already printed, line left as typed
    rl_buffer:refreshline()
end

function comma_enter_cr(rl_buffer)
    comma_enter(rl_buffer, ENTER_KEYS[1])
end

function comma_enter_lf(rl_buffer)
    comma_enter(rl_buffer, ENTER_KEYS[2])
end

-- Readline bindings outlive a script reload; don't save our own binding as the
-- "original", or Enter would call itself.
local funcs = { [["luafunc:comma_enter_cr"]], [["luafunc:comma_enter_lf"]] }
for _, m in ipairs({ "emacs", "vi-insert", "vi-command" }) do
    original[m] = {}
    for i, k in ipairs(ENTER_KEYS) do
        local b = rl.getbinding(k, m)
        if not b or b:find("luafunc:comma_enter", 1, true) then
            b = "accept-line"
        end
        original[m][k] = b
        rl.setbinding(k, funcs[i], m)
    end
end
