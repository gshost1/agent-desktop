#!/usr/bin/env python3
"""Shared-desktop lock (~/.agent-desktop/bench.lock). Usage: lock.py take|release TITLE."""
import os
import sys
import time
from pathlib import Path

LOCK = Path.home() / ".agent-desktop" / "bench.lock"


def take_lock(title, poll=60):
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
    if LOCK.exists() and title in LOCK.read_text():
        LOCK.unlink()


if __name__ == "__main__":
    {"take": take_lock, "release": release_lock}[sys.argv[1]](sys.argv[2])
