#!/usr/bin/env python3
"""Shared-desktop lock (~/.agent-desktop/bench.lock). Usage: lock.py take|release TITLE.

When the shared waiting-list script exists (BENCH_LOCKQ, default the speed session's
bench/lockq.sh), take joins its queue and waits for its turn with a heartbeat, and
release goes through it; otherwise the plain lock file is used.
"""
import os
import subprocess
import sys
import time
from pathlib import Path

LOCK = Path.home() / ".agent-desktop" / "bench.lock"
LOCKQ = Path(os.environ.get("BENCH_LOCKQ", "/Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/lockq.sh"))


def take_lock(title, poll=int(os.environ.get("BENCH_LOCK_POLL", "5"))):
    if LOCKQ.is_file():
        subprocess.run(["bash", str(LOCKQ), "take", title, str(poll)], check=True)
        return
    while True:
        if LOCK.exists():
            age = time.time() - LOCK.stat().st_mtime
            if age < 7200 and title not in LOCK.read_text():
                print(f"lock held ({int(age)}s): {LOCK.read_text().strip()}", flush=True)
                time.sleep(poll)
                continue
        fd = os.open(LOCK, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        os.write(fd, f"{title}\n{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}\n".encode())
        os.close(fd)
        return


def release_lock(title):
    if LOCKQ.is_file():
        subprocess.run(["bash", str(LOCKQ), "release", title], check=True)
        return
    if LOCK.exists() and title in LOCK.read_text():
        LOCK.unlink()


if __name__ == "__main__":
    {"take": take_lock, "release": release_lock}[sys.argv[1]](sys.argv[2])
