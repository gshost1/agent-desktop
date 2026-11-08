#!/usr/bin/env bash
# Run right after taking the desktop lock: blocks until no serial_cargo build is
# running. New builds re-check the desktop lock under the build lock and wait.
exec python3 -I -c '
import fcntl, os
with open(os.path.expanduser("~/.agent-desktop-bench-build.lock"), "w") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
'
