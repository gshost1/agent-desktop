#!/usr/bin/env bash
# Runs benchmark rounds one at a time and shares the desktop between rounds.
#
# Each line of the plan file is one round: a label, then space-separated
# task:config:model[:tree] entries ("extras" runs bench/run_extras.sh, sh:<script>
# runs a script). A line "wait-browser" waits until feat/unified-browser bench
# has taken and released the lock, or 60 min. The lock is held for one round and
# released after it. A background observer records when any other session's
# title was last in ~/.agent-desktop/bench.lock; the lock is not retaken while
# another holder was seen in the last 10 minutes, so a session that polls
# slowly still gets every other round.
#
# Usage: bash bench/orchestrate.sh bench/plan.txt
set -u
cd "$(dirname "$0")/.."
plan="$1"
lock="$HOME/.agent-desktop/bench.lock"
owner="perf/speed-cost bench (agent-desktop worktree perf-speed-cost)"

log() { echo "[$(date +%H:%M:%S)] $*"; }

lock_age_s() {
    local stamp
    stamp=$(sed -n 2p "$lock" 2>/dev/null)
    [ -n "$stamp" ] || { echo 0; return; }
    echo $(( $(date -u +%s) - $(date -u -j -f "%Y-%m-%dT%H:%M:%SZ" "$stamp" +%s 2>/dev/null || date -u +%s) ))
}

write_lock() { printf '%s\n%s\n' "$owner" "$(date -u +%FT%TZ)" > "$lock"; }

seen_dir=bench/out/lock-seen
mkdir -p "$seen_dir"
started_at=$(date +%s)

observe_lock() {
    while true; do
        if [ -f "$lock" ] && ! grep -q "perf/speed-cost" "$lock" 2>/dev/null; then
            date +%s > "$seen_dir/other"
            head -1 "$lock" 2>/dev/null > "$seen_dir/other_title"
            grep -q "feat/unified-browser" "$lock" 2>/dev/null && date +%s > "$seen_dir/browser"
        fi
        sleep 1
    done
}

other_seen_within() {
    local last
    last=$(cat "$seen_dir/other" 2>/dev/null || echo 0)
    [ $(( $(date +%s) - last )) -lt "$1" ]
}

held_by_other() {
    [ -f "$lock" ] && ! grep -q "perf/speed-cost" "$lock" && [ "$(lock_age_s)" -lt 7200 ]
}

take_lock() {
    while held_by_other || other_seen_within "$yield_s"; do sleep 10; done
    write_lock
    log "lock taken"
}

release_lock() {
    if grep -q "perf/speed-cost" "$lock" 2>/dev/null; then rm -f "$lock"; fi
    log "lock released; not retaking while another holder was seen in the last $((yield_s / 60)) min"
}

wait_for_browser_round() {
    log "waiting for feat/unified-browser bench to take and release the lock (at most 60 min)"
    while [ $(( $(date +%s) - started_at )) -lt 3600 ]; do
        if [ -f "$seen_dir/browser" ] && ! grep -q "feat/unified-browser" "$lock" 2>/dev/null; then
            log "browser round seen and released"
            return
        fi
        sleep 10
    done
    log "60 min passed without a browser round"
}

yield_s=600
rm -f "$seen_dir/other" "$seen_dir/browser"
observe_lock &
observer=$!
trap 'kill $observer 2>/dev/null' EXIT

while IFS= read -r line; do
    [ -n "$line" ] || continue
    case "$line" in \#*) continue ;; esac
    if [ "$line" = "wait-browser" ]; then wait_for_browser_round; continue; fi
    label="${line%% *}"
    entries="${line#* }"
    take_lock
    log "round $label start"
    for entry in $entries; do
        write_lock
        if [ "$entry" = "extras" ]; then
            bash bench/run_extras.sh
            continue
        fi
        case "$entry" in sh:*) bash "${entry#sh:}"; continue ;; esac
        IFS=: read -r task cfg model tree <<< "$entry"
        echo "== round $label task $task config $cfg model $model tree ${tree:-bench}"
        python3 bench/driver.py --task "$task" --config "$cfg" --model "$model" ${tree:+--tree "$tree"} || echo "driver failed"
    done
    log "round $label done"
    release_lock
done < "$plan"
log "plan complete"
