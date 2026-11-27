"""Type keystrokes into a real cmd.exe with Clink through a ConPTY; print what it shows.

Usage: python -I conpty_drive.py PROFILE_DIR CHUNK...
Waits for the "<<<READY>>>" line a profile script prints once Clink's editor
starts on the first line. Each CHUNK is sent, then output is read until the console goes
quiet. A CHUNK of the form "keys\x1ftext" also waits until text appears. Each
chunk's output is preceded by a line "<<<CHUNK n>>>" so callers can tell which
keystrokes produced what. ANSI/OSC sequences are stripped.
Clink is taken from %CLINK_EXE%, else the default install path.
"""
import os, queue, re, sys, threading, time
from winpty import PtyProcess

profile, chunks = sys.argv[1], sys.argv[2:]
clink = os.environ.get("CLINK_EXE", r"C:\Program Files (x86)\clink\clink_x64.exe")
# /d skips AutoRun, so an installed Clink autorun can't load the user's profile.
# pywinpty joins argv with MSVC quoting; the leading "call" stops cmd /k from
# stripping the quotes around a Clink path that contains "(x86)".
argv = ["cmd.exe", "/d", "/k", "call", clink, "inject", "--quiet", "--profile", profile]
p = PtyProcess.spawn(argv, cwd=os.getcwd(), env=dict(os.environ), dimensions=(50, 250))

q = queue.Queue()
def reader():
    while True:
        try:
            q.put(p.read(4096))
        except EOFError:
            q.put(None)
            return
threading.Thread(target=reader, daemon=True).start()

def drain(quiet=1.5, limit=15.0, want=""):
    out, end, last = "", time.time() + limit, time.time()
    while time.time() < end and (time.time() - last < quiet or want not in clean(out)):
        try:
            s = q.get(timeout=0.05)
        except queue.Empty:
            continue
        if s is None:
            break
        out, last = out + s, time.time()
    return out

ansi = re.compile(r"\x1b\][^\x07\x1b]*(\x07|\x1b\\)|\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[=>]")
def clean(s):
    return ansi.sub("", s).replace("\r", "")

out = clean(drain(quiet=1.0, limit=60.0, want="<<<READY>>>"))
for i, c in enumerate(chunks, 1):
    keys, _, want = c.partition("\x1f")
    p.write(keys)
    out += f"\n<<<CHUNK {i}>>>\n" + clean(drain(want=want))
p.write("exit\r")
drain(quiet=0.5, limit=3.0)
if p.isalive():
    p.terminate(force=True)
sys.stdout.write(out)
