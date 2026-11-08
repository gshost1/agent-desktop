#!/usr/bin/env python3
"""Live check that TextEdit's Save panel opens in the bench folder once
point_save_folder has run, and that the original pref comes back afterwards,
also when the process that pointed it dies before cleanup.

Prints only the Save panel's "Where:" value, never a folder listing. Presses
Escape at once when "Where:" is not the bench folder. Run under the desktop lock.

Usage: python3 bench/check_save_folder.py <agent-desktop binary>
"""
import json
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
from tasks import (BENCH_DIR, SAVE_FOLDER_ORIGINAL, macctl, point_save_folder, quit_app,  # noqa: E402
                   read_save_folder, restore_save_folder)


def where_value(binary):
    done = subprocess.run([binary, "find", "--app", "TextEdit", "--surface", "sheet", "--name", "Where:", "--first"],
                          capture_output=True, text=True, timeout=30)
    try:
        matches = json.loads(done.stdout).get("data", {}).get("matches", [])
    except json.JSONDecodeError:
        return None
    return matches[0].get("value") if matches else None


def one_check(binary, attempt):
    folder = BENCH_DIR / f"bench-savefolder-{uuid.uuid4().hex[:6]}"
    folder.mkdir(parents=True)
    before = read_save_folder()
    try:
        point_save_folder(folder)
        subprocess.run(["open", "-a", "TextEdit"], check=True, timeout=10)
        time.sleep(2.5)
        macctl("activate", "TextEdit")
        macctl("key", "TextEdit", "45", "cmd")
        time.sleep(1.5)
        macctl("key", "TextEdit", "1", "cmd")
        time.sleep(2.5)
        where = where_value(binary)
        macctl("key", "TextEdit", "53")
        ok = where == folder.name
        print(json.dumps({"attempt": attempt, "where": where if ok else "(not the bench folder; not shown)",
                          "expected": folder.name, "panel_in_bench_folder": ok}))
    finally:
        quit_app("TextEdit")
        restore_save_folder()
        shutil.rmtree(folder, ignore_errors=True)
    after = read_save_folder()
    print(json.dumps({"attempt": attempt, "pref_before": before, "pref_after": after, "restored": before == after}))
    return ok and before == after


def crash_check():
    folder = BENCH_DIR / f"bench-savefolder-{uuid.uuid4().hex[:6]}"
    folder.mkdir(parents=True)
    before = read_save_folder()
    code = ("import sys; sys.path.insert(0, %r); import os, tasks; tasks.point_save_folder(%r); os._exit(9)"
            % (str(ROOT), str(folder)))
    subprocess.run([sys.executable, "-c", code], timeout=60)
    pending = SAVE_FOLDER_ORIGINAL.exists()
    mid = read_save_folder()
    restore_save_folder()
    after = read_save_folder()
    shutil.rmtree(folder, ignore_errors=True)
    print(json.dumps({"crash": True, "pending_after_kill": pending, "pref_while_pending": mid,
                      "pref_before": before, "pref_after": after, "restored": before == after}))
    return pending and before == after


def main():
    binary = sys.argv[1]
    results = [one_check(binary, 1), one_check(binary, 2), crash_check()]
    print("ALL PASS" if all(results) else "FAIL")


if __name__ == "__main__":
    main()
