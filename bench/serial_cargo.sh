#!/usr/bin/env bash
# Runs one cargo command at a time across every worktree on this Mac, at low
# priority, in the shared bench target dir, and never while the desktop lock is
# held for a measured round or a live check. Parallel test builds froze the Mac
# on 2026-11-07 (load 36 on 8 cores, swap nearly full).
#
# Usage, from any checkout:
#   bash /Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/serial_cargo.sh cargo test -p agent-desktop-macos --lib
set -u
[ $# -gt 0 ] || { echo "usage: serial_cargo.sh <command> [args]" >&2; exit 2; }
export CARGO_TARGET_DIR="$HOME/.agent-desktop-bench-target"
export CARGO_BUILD_JOBS=2
export RUST_TEST_THREADS=2
desktop_lock="$HOME/.agent-desktop/bench.lock"
until [ -z "$(head -1 "$desktop_lock" 2>/dev/null)" ]; do
    echo "serial_cargo: desktop lock held by '$(head -1 "$desktop_lock")', waiting" >&2
    sleep 20
done
exec python3 -I -c '
import fcntl, os, subprocess, sys, time
desktop = os.path.expanduser("~/.agent-desktop/bench.lock")
def desktop_holder():
    try:
        with open(desktop) as fh:
            return fh.readline().strip()
    except OSError:
        return ""
with open(os.path.expanduser("~/.agent-desktop-bench-build.lock"), "w") as lock:
    print("serial_cargo: waiting for the build lock", file=sys.stderr, flush=True)
    while True:
        fcntl.flock(lock, fcntl.LOCK_EX)
        holder = desktop_holder()
        if not holder:
            break
        fcntl.flock(lock, fcntl.LOCK_UN)
        print(f"serial_cargo: desktop lock held by {holder!r}, waiting", file=sys.stderr, flush=True)
        time.sleep(20)
    print("serial_cargo: build lock taken", file=sys.stderr, flush=True)
    target = os.environ["CARGO_TARGET_DIR"]
    used = subprocess.run(["du", "-sk", target], capture_output=True, text=True).stdout.split()
    if used and int(used[0]) > 20 * 1024 * 1024:
        print(f"serial_cargo: {target} is over 20 GB; cleaning it under the build lock", file=sys.stderr, flush=True)
        subprocess.call(["rm", "-rf", target])
    sys.exit(subprocess.call(["nice", "-n", "20", *sys.argv[1:]]))
' "$@"
