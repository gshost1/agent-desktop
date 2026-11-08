#!/usr/bin/env bash
# Runs one cargo command at a time across every worktree on this Mac, at low
# priority, in the shared bench target dir, and never while the desktop lock is
# held for a measured round or a live check. Parallel test builds froze the Mac
# on 2026-11-07 (load 36 on 8 cores, swap nearly full).
#
#
# Each checkout gets its own target dir under ~/.agent-desktop-bench-target. One
# dir shared across worktrees gave wrong builds: cargo fingerprints workspace
# path crates by paths relative to the workspace root, so worktree B reused
# worktree A's agent-desktop-core (2026-11-08).
#
# Every command is stopped after BENCH_CARGO_TIMEOUT_S (default 3600 s): a hung
# test held the build lock, which the measured rounds wait on (2026-11-08).
#
# Usage, from any checkout:
#   bash /Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/serial_cargo.sh cargo test -p agent-desktop-macos --lib
#   bash .../serial_cargo.sh --target-dir      prints this checkout's target dir
set -u
[ $# -gt 0 ] || { echo "usage: serial_cargo.sh <command> [args]" >&2; exit 2; }
bench_targets="$HOME/.agent-desktop-bench-target"
top="$(git rev-parse --show-toplevel 2>/dev/null || pwd -P)"
top="$(cd "$top" && pwd -P)"
export CARGO_TARGET_DIR="$bench_targets/trees/$(basename "$top")-$(printf %s "$top" | shasum | cut -c1-8)"
if [ "$1" = "--target-dir" ]; then echo "$CARGO_TARGET_DIR"; exit 0; fi
mkdir -p "$CARGO_TARGET_DIR"
printf '%s\n' "$top" > "$CARGO_TARGET_DIR/.source-tree"
export CARGO_BUILD_JOBS=2
export RUST_TEST_THREADS=2
desktop_lock="$HOME/.agent-desktop/bench.lock"
until [ -z "$(head -1 "$desktop_lock" 2>/dev/null)" ]; do
    echo "serial_cargo: desktop lock held by '$(head -1 "$desktop_lock")', waiting" >&2
    sleep 20
done
export BENCH_SOURCE_TREE="$top"
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
    trees = os.path.dirname(target)
    for name in os.listdir(trees):
        other = os.path.join(trees, name)
        try:
            with open(os.path.join(other, ".source-tree")) as fh:
                source = fh.readline().strip()
        except OSError:
            continue
        if other != target and source and not os.path.isdir(source):
            print(f"serial_cargo: {source} is gone; removing its target {other}", file=sys.stderr, flush=True)
            subprocess.call(["rm", "-rf", other])
    used = subprocess.run(["du", "-sk", target], capture_output=True, text=True).stdout.split()
    if used and int(used[0]) > 20 * 1024 * 1024:
        print(f"serial_cargo: {target} is over 20 GB; cleaning it under the build lock", file=sys.stderr, flush=True)
        subprocess.call(["rm", "-rf", target])
        os.makedirs(target, exist_ok=True)
        with open(os.path.join(target, ".source-tree"), "w") as fh:
            fh.write(os.environ["BENCH_SOURCE_TREE"] + "\n")
    limit = os.environ.get("BENCH_CARGO_TIMEOUT_S", "3600")
    code = subprocess.call(["/opt/homebrew/bin/timeout", "--kill-after=30", limit, "nice", "-n", "20", *sys.argv[1:]])
    if code == 124:
        print(f"serial_cargo: command passed {limit} s and was stopped; a hung command would hold the build lock and stall the desktop", file=sys.stderr, flush=True)
    sys.exit(code)
' "$@"
