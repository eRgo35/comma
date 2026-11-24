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
local original = {}

-- Everything reaches comma through the environment: io.popen runs its command
-- line through cmd, which would act on "&", ">", "%VAR%" in the request.
local function ask(line)
    os.setenv("COMMA_REQUEST", line:sub(2))
    os.setenv("COMMA_SHELL", "cmd")
    os.setenv("COMMA_PREV_REQUEST", prev_request)
    os.setenv("COMMA_PREV_COMMAND", prev_command)
    os.setenv("COMMA_API_KEY", key)
    local f = io.popen("comma ask")
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

function comma_enter(rl_buffer)
    local line = rl_buffer:getbuffer()
    if line:sub(1, 1) ~= "," then
        local mode = rl.getvariable("editing-mode") == "vi" and "vi-insert" or "emacs"
        rl.invokecommand(original[mode] or "accept-line")
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

-- Readline bindings outlive a script reload; don't save our own binding as the
-- "original", or Enter would call itself.
for _, m in ipairs({ "emacs", "vi-insert", "vi-command" }) do
    local b = rl.getbinding([["\r"]], m)
    if not b or b:find("luafunc:comma_enter", 1, true) then
        b = "accept-line"
    end
    original[m] = b
    rl.setbinding([["\r"]], [["luafunc:comma_enter"]], m)
end
