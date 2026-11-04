"""Type keystrokes into a real interactive bash through a pty; print what it shows.

Usage: python3 -I pty_drive.py HOOK_FILE CHUNK...
Each CHUNK is sent, then output is read until bash goes quiet.
"""
import os, pty, select, sys, time

hook, chunks = sys.argv[1], sys.argv[2:]
pid, fd = pty.fork()
if pid == 0:
    os.environ["PS1"] = "$ "
    os.execvp("bash", ["bash", "--norc", "--noprofile", "-i"])

def drain(quiet=0.4, limit=5.0):
    out, end, last = b"", time.time() + limit, time.time()
    while time.time() < end and time.time() - last < quiet:
        if select.select([fd], [], [], 0.05)[0]:
            try:
                out += os.read(fd, 4096)
            except OSError:
                break
            last = time.time()
    return out

out = drain()
for c in [f'source "{hook}"\r'] + chunks:
    os.write(fd, c.encode())
    out += drain()
os.write(fd, b"\rexit\r")
out += drain()
sys.stdout.write(out.decode(errors="replace").replace("\r", ""))
