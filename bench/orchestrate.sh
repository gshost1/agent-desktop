#!/usr/bin/env bash
# Runs benchmark rounds one at a time and shares the desktop between rounds.
#
# Each line of the plan file is one round: a label, then space-separated
# task:config:model[:tree] entries ("extras" runs bench/run_extras.sh, sh:<script>
# runs a script). The lock is held for one round and released after it. Every
# take goes through the shared waiting list in bench/lockq.sh, so a session
# that is already waiting gets the next round.
#
# A line "wait-browser" waits, with no time limit, until feat/unified-browser
# bench has held the lock and released it.
#
# The plan is read on fd 3 and every child gets /dev/null as stdin: a child that
# reads stdin would otherwise consume the plan file and end the plan early.
#
# Before each round it checks free space on the data volume. Under
# BENCH_MIN_FREE_GB (15) it does not start the round: it logs a DISK_LOW line
# and checks again every 2 minutes, without holding the desktop lock. A full
# disk once ran rounds for two hours that wrote no results.
#
# Usage: bash bench/orchestrate.sh bench/plan.txt [pid to wait for first]
set -u
cd "$(dirname "$0")/.."
plan="$1"
previous="${2:-}"
lock="$HOME/.agent-desktop/bench.lock"
owner="perf/speed-cost bench (agent-desktop worktree perf-speed-cost)"

data_volume=/System/Volumes/Data
min_free_gb="${BENCH_MIN_FREE_GB:-15}"
disk_retry_s=120

log() { echo "[$(date +%H:%M:%S)] $*"; }

free_gb() { df -k "$data_volume" | awk 'NR == 2 { print int($4 / 1048576) }'; }

wait_for_disk() {
    local free paused=0
    while free=$(free_gb); [ "${free:-0}" -lt "$min_free_gb" ]; do
        log "DISK_LOW ${free:-?}G free on $data_volume, under ${min_free_gb}G: round $1 not started, retrying in ${disk_retry_s}s"
        paused=1
        sleep "$disk_retry_s"
    done
    [ "$paused" = 1 ] && log "disk ok: ${free}G free, round $1 resumes"
    return 0
}

holder() { head -1 "$lock" 2>/dev/null; }

refresh_lock() {
    [ "$(holder)" = "$owner" ] && printf '%s\n%s\n' "$owner" "$(date -u +%FT%TZ)" > "$lock"
    return 0
}

take_lock() {
    log "waiting in bench.queue: $(bash bench/lockq.sh status | tr '\n' ' ')"
    bash bench/lockq.sh take "$owner" 5 >/dev/null
    log "lock taken"
}

release_lock() {
    bash bench/lockq.sh release "$owner"
    log "lock released"
}

wait_for_browser_round() {
    log "waiting for feat/unified-browser bench to hold and release the lock (no time limit)"
    until holder | grep -q "feat/unified-browser"; do sleep 2; done
    log "browser round started"
    while holder | grep -q "feat/unified-browser"; do sleep 2; done
    log "browser round released"
}

if [ -n "$previous" ]; then
    log "waiting for the previous orchestrator ($previous) to finish its round"
    while kill -0 "$previous" 2>/dev/null; do sleep 5; done
fi

while IFS= read -r line <&3; do
    [ -n "$line" ] || continue
    case "$line" in \#*) continue ;; esac
    if [ "$line" = "wait-browser" ]; then wait_for_browser_round; continue; fi
    label="${line%% *}"
    entries="${line#* }"
    wait_for_disk "$label"
    take_lock
    bash bench/wait_builds_idle.sh
    log "round $label start"
    for entry in $entries; do
        refresh_lock
        if [ "$entry" = "extras" ]; then
            bash bench/run_extras.sh < /dev/null
            continue
        fi
        case "$entry" in sh:*) bash "${entry#sh:}" < /dev/null; continue ;; esac
        IFS=: read -r task cfg model tree <<< "$entry"
        echo "== round $label task $task config $cfg model $model tree ${tree:-bench}"
        python3 bench/driver.py --task "$task" --config "$cfg" --model "$model" ${tree:+--tree "$tree"} < /dev/null || echo "driver failed"
    done
    log "round $label done"
    release_lock
done 3< "$plan"
log "plan complete"
