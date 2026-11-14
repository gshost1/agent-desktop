#!/bin/bash
# One candidate agent-desktop arm on the remote bench Mac: rounds 0..N-1 of the given held-out tasks, the same
# task rotation, seeds (web: by round), prompts, caps and Chrome flags as remote_run_all.sh. Runs inside the
# bench account's GUI session (cand_start.sh starts it through benchrun). After every run it checks the API
# refusal scanner, a STOP file, and this pass's list-price spend; past CAP_USD it writes STOP-SPEND and ends.
#
#   bash cand_pass.sh <label> <task ids, comma-separated or "all"> <n> [model]
#
# The candidate binary is ~/.agent-desktop-bench-target/heldout-tools/candidates/<label>/ (bench HOME), put
# there by bench_candidate.sh with its helper and a VERSION line; common.register_candidates() makes it an arm.
set -u
BT=$BENCH_REAL_HOME/bench-tools
H=$HOME/.t3/worktrees/agent-desktop/perf-speed-cost/bench/heldout
B=$(dirname "$H")
P=$HELDOUT_PY
LABEL=$1; TASKS=${2:-all}; N=${3:-5}; MODEL=${4:-claude-sonnet-5-5}; CAP_USD=${CAP_USD:-15}
HOST=${HELDOUT_HOST:-friend-m1pro}
OWNER="perf/speed-cost heldout"
CHROME_ARGS=(--chrome-arg=--use-mock-keychain --chrome-arg=--password-store=basic)
ALL_NATIVE=(hn1 hn2 hn3 hn4 hn5 hn6 hn7 hn8)
ALL_WEB=(hw1 hw2 hw3 hw4 hw5 hw6 hw7 hp1 hp2 hp3)
NATIVE_ROWS=$H/runs/results-native.jsonl
WEB_COSTS=$H/runs/web/$LABEL-$MODEL/runs.jsonl

[ -x "$HOME/.agent-desktop-bench-target/heldout-tools/candidates/$LABEL/agent-desktop" ] || { echo "no candidate $LABEL"; exit 2; }
frozen=$(grep -oE 'tasks_heldout.py sha256 `[0-9a-f]{64}`' "$H/TASKS_FROZEN.md" | grep -oE '[0-9a-f]{64}')
actual=$(shasum -a 256 "$H/tasks_heldout.py" | cut -d' ' -f1)
[ -n "$frozen" ] && [ "$frozen" = "$actual" ] || { echo "tasks_heldout.py sha256 $actual does not match the frozen $frozen"; exit 3; }

NATIVE_TASKS=(); WEB_TASKS=()
for t in "${ALL_NATIVE[@]}"; do [[ $TASKS == all || ",$TASKS," == *",$t,"* ]] && NATIVE_TASKS+=("$t"); done
for t in "${ALL_WEB[@]}"; do [[ $TASKS == all || ",$TASKS," == *",$t,"* ]] && WEB_TASKS+=("$t"); done

rot() { local n=$1; shift; local a=("$@"); local k=$(( n % ${#a[@]} )); echo "${a[@]:$k}" "${a[@]:0:$k}"; }
log() { echo "[$(date +%H:%M:%S)] $*"; }
lines() { [ -f "$1" ] && wc -l < "$1" | tr -d ' ' || echo 0; }
NATIVE_FROM=$(lines "$NATIVE_ROWS"); WEB_FROM=$(lines "$WEB_COSTS")
spent() {
  $P -I - "$NATIVE_ROWS" "$NATIVE_FROM" "$WEB_COSTS" "$WEB_FROM" "$LABEL" <<'PY'
import json, sys
from pathlib import Path
native, nfrom, web, wfrom, label = sys.argv[1], int(sys.argv[2]), sys.argv[3], int(sys.argv[4]), sys.argv[5]
def new(path, start):
    p = Path(path)
    return [json.loads(l) for l in p.read_text().splitlines()[start:] if l.strip()] if p.exists() else []
cost = sum(float(r.get("cost_usd_list") or 0) for r in new(native, nfrom) if r.get("arm") == label)
cost += sum(float(r.get("cost_usd_list") or 0) for r in new(web, wfrom))
print(round(cost, 4))
PY
}
check() {
  if [ -f "$BT/STOP" ]; then log "STOP file found, ending the pass"; return 0; fi
  if ! $P -I "$BT/tools/api_refusal.py" "$H/runs" "$BT/logs/.since" > "$BT/logs/api_refusal.txt"; then
    log "API REFUSED: $(cat "$BT/logs/api_refusal.txt")"; touch "$BT/STOP-API"; return 0
  fi
  local s; s=$(spent)
  log "$LABEL spend so far \$$s (cap \$$CAP_USD)"
  if $P -I -c 'import sys; sys.exit(0 if float(sys.argv[1]) > float(sys.argv[2]) else 1)' "$s" "$CAP_USD"; then
    log "SPEND CAP: $LABEL spend \$$s passed \$$CAP_USD"; touch "$BT/STOP-SPEND"; return 0
  fi
  return 1
}

caffeinate -dims -w $$ &
server_pid=""
if [ ${#WEB_TASKS[@]} -gt 0 ] && ! $P -I -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8792/__state', timeout=3)" 2>/dev/null; then
  $P -I "$H/web/server.py" 8792 > "$H/out/server.log" 2>&1 &
  server_pid=$!
  sleep 1
fi
trap '[ -n "$server_pid" ] && kill "$server_pid" 2>/dev/null; bash "$B/lockq.sh" release "$OWNER" > /dev/null 2>&1' EXIT

log "CAND START $LABEL model $MODEL rounds 0-$((N - 1)) native ${NATIVE_TASKS[*]:-none} web ${WEB_TASKS[*]:-none}"
for (( r=0; r<N; r++ )); do
  if [ ${#NATIVE_TASKS[@]} -gt 0 ]; then
    bash "$B/lockq.sh" take "$OWNER" 5 > /dev/null
    for task in $(rot "$r" "${NATIVE_TASKS[@]}"); do
      touch "$BT/logs/.since"
      $P -I "$H/arms/run_native.py" --arm "$LABEL" --task "$task" --round "$r" --model "$MODEL" --host "$HOST" < /dev/null \
        || { log "native $LABEL $task exited; retrying once"
             $P -I "$H/arms/run_native.py" --arm "$LABEL" --task "$task" --round "$r" --model "$MODEL" --host "$HOST" < /dev/null \
               || log "native $LABEL $task exited again"; }
      if check; then exit 9; fi
    done
    bash "$B/lockq.sh" release "$OWNER" > /dev/null
  fi
  if [ ${#WEB_TASKS[@]} -gt 0 ]; then
    for task in $(rot "$r" "${WEB_TASKS[@]}"); do
      touch "$BT/logs/.since"
      $P -I "$H/arms/run_web.py" --arm "$LABEL" --tasks "$task" --runs 1 --start-run "$r" --model "$MODEL" --host "$HOST" \
        --timeout 600 "${CHROME_ARGS[@]}" < /dev/null || log "web $LABEL $task round $r exited $?"
      if check; then exit 9; fi
    done
  fi
  log "round $r done"
done
log "CAND COMPLETE $LABEL spend \$$(spent)"
