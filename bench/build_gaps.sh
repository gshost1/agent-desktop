#!/usr/bin/env bash
# Gives queued builds a turn between measured rounds. Measured rounds hold the
# desktop lock back to back, and serial_cargo waits while it is held, so writer
# builds could starve all night. When a serial_cargo build has waited over two
# minutes, this sets ~/.agent-desktop/bench.pause (lockq grants no new lock while
# it is fresh), waits for the current round to release the desktop, lets every
# queued build run, then clears the pause. A gap lasts at most 25 minutes, and
# the next gap waits until another round has run.
set -u
dir="$HOME/.agent-desktop"; pause="$dir/bench.pause"; lock="$dir/bench.lock"
log() { echo "[$(date +%H:%M:%S)] $*"; }
waiters() { pgrep -f 'desktop_holder' 2>/dev/null; }
oldest_wait_s() {
    local pids
    pids=$(waiters | tr '\n' ' ')
    [ -n "$pids" ] || { echo 0; return; }
    ps -o etime= -p ${pids// /,} 2>/dev/null | python3 -c '
import sys
best = 0
for line in sys.stdin:
    t = line.strip()
    if not t:
        continue
    days, _, rest = t.rpartition("-")
    parts = [int(x) for x in rest.split(":")]
    while len(parts) < 3:
        parts.insert(0, 0)
    best = max(best, int(days or 0) * 86400 + parts[0] * 3600 + parts[1] * 60 + parts[2])
print(best)'
}
building() { pgrep -f 'cargo (build|test|clippy|check)|rustc ' >/dev/null; }
trap 'rm -f "$pause"' EXIT
while true; do
    if [ "$(oldest_wait_s)" -gt 120 ] && [ -s "$lock" ]; then
        log "builds waiting; pausing new desktop turns"
        printf 'build-gap\n' > "$pause"
        until [ ! -s "$lock" ]; do touch "$pause"; sleep 10; done
        log "desktop free; builds running"
        start=$(date +%s)
        quiet=0
        while [ $(( $(date +%s) - start )) -lt 1500 ]; do
            touch "$pause"
            if [ -z "$(waiters)" ] && ! building; then quiet=$((quiet + 10)); else quiet=0; fi
            [ "$quiet" -ge 120 ] && break
            sleep 10
        done
        rm -f "$pause"
        log "gap over after $(( $(date +%s) - start )) s"
        until [ -s "$lock" ]; do sleep 15; done
        until [ ! -s "$lock" ]; do sleep 15; done
    fi
    sleep 30
done
