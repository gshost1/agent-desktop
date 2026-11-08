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
# A round takes the desktop lock only when every binary it measures is built and
# passes check_binary_tree, and only when no serial_cargo build holds the build
# lock. It never waits on the build lock while it holds the desktop lock: a hung
# test once held the build lock while a round held the desktop (2026-11-08). A
# round with a missing binary is requeued after the rest of the plan, up to 3
# more passes 5 minutes apart, then skipped.
#
# Usage: bash bench/orchestrate.sh bench/plan.txt bench/plan.txt [pid to wait for first]
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

builds_idle_now() {
    python3 -I -c '
import fcntl, os, sys
with open(os.path.expanduser("~/.agent-desktop-bench-build.lock"), "w") as lock:
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        sys.exit(1)
'
}

missing_binary() {
    local entry task cfg model tree
    for entry in $1; do
        case "$entry" in extras|sh:*) continue ;; esac
        IFS=: read -r task cfg model tree <<< "$entry"
        [ -n "$tree" ] || continue
        if [ ! -x "$tree/target/release/agent-desktop" ] || ! bash bench/check_binary_tree.sh "$tree" >/dev/null 2>&1; then
            echo "$tree"
            return 0
        fi
    done
    return 1
}

take_lock_with_builds_idle() {
    while :; do
        bash bench/wait_builds_idle.sh
        take_lock
        builds_idle_now && return 0
        log "a build started before the desktop lock was taken; releasing the desktop until it ends"
        release_lock
        sleep 5
    done
}

run_round() {
    local label="$1" entries="$2" entry task cfg model tree
    wait_for_disk "$label"
    take_lock_with_builds_idle
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
        if [ -n "$tree" ] && ! bash bench/check_binary_tree.sh "$tree"; then
            echo "round $label: binary in $tree is not from its own tree; entry skipped"
            continue
        fi
        python3 bench/driver.py --task "$task" --config "$cfg" --model "$model" ${tree:+--tree "$tree"} < /dev/null || echo "driver failed"
    done
    log "round $label done"
    release_lock
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

deferred=()
while IFS= read -r line <&3; do
    [ -n "$line" ] || continue
    case "$line" in \#*) continue ;; esac
    if [ "$line" = "wait-browser" ]; then wait_for_browser_round; continue; fi
    label="${line%% *}"
    entries="${line#* }"
    if missing=$(missing_binary "$entries"); then
        log "round $label: binary in $missing is missing or not from its own tree; requeued after the plan"
        deferred+=("$line")
        continue
    fi
    run_round "$label" "$entries"
done 3< "$plan"
for pass in 1 2 3; do
    [ "${#deferred[@]}" -gt 0 ] || break
    log "requeue pass $pass: ${#deferred[@]} round(s), checking again in 300 s"
    sleep 300
    still=()
    for line in "${deferred[@]}"; do
        label="${line%% *}"
        entries="${line#* }"
        if missing=$(missing_binary "$entries"); then
            still+=("$line")
            continue
        fi
        run_round "$label" "$entries"
    done
    deferred=("${still[@]+"${still[@]}"}")
done
for line in "${deferred[@]+"${deferred[@]}"}"; do log "round ${line%% *}: SKIPPED, binary still missing after 3 requeue passes"; done
log "plan complete"
