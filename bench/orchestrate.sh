#!/usr/bin/env bash
# Runs benchmark rounds one at a time and shares the desktop between rounds.
#
# Each line of the plan file is one round: a label, then space-separated
# task:config:model[:tree] entries (or "extras" for bench/run_extras.sh, or
# sh:<script> to run a script). A line "yield" only hands the lock over. Before a
# round it takes ~/.agent-desktop/bench.lock (waiting while another session
# holds a lock younger than 2 h). After a round it removes the lock, waits for
# another session to take it and release it again (gives up after 60 min of
# no take, or 2 h of a held lock), then continues.
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

take_lock() {
    while [ -f "$lock" ] && ! grep -q "perf/speed-cost" "$lock" && [ "$(lock_age_s)" -lt 7200 ]; do
        sleep 20
    done
    write_lock
    log "lock taken"
}

release_and_yield() {
    if grep -q "perf/speed-cost" "$lock" 2>/dev/null; then rm -f "$lock"; fi
    log "lock released; waiting up to 60 min for another session's round"
    local waited=0
    while [ ! -f "$lock" ] && [ "$waited" -lt 3600 ]; do sleep 15; waited=$((waited + 15)); done
    if [ ! -f "$lock" ]; then log "nobody took the lock in 60 min"; return; fi
    log "another session holds the lock: $(head -1 "$lock")"
    while [ -f "$lock" ] && ! grep -q "perf/speed-cost" "$lock" && [ "$(lock_age_s)" -lt 7200 ]; do sleep 20; done
    log "other session released the lock"
}

while IFS= read -r line; do
    [ -n "$line" ] || continue
    case "$line" in \#*) continue ;; esac
    if [ "$line" = "yield" ]; then release_and_yield; continue; fi
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
    release_and_yield
done < "$plan"
log "plan complete"
