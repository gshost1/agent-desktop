#!/bin/bash
# Several native arms interleaved on the remote bench Mac, for a same-day comparison on one frozen task set.
#
#   bash arms_pass.sh <pass name> <arms, comma-separated> <native task ids|all> <n> <model> <taskset> [rules]
#
# Round r runs the task list rotated by r. Task t (its index in the set's list) runs its arms in the order rotated
# by r + t, so over k rounds of k arms every arm takes every slot of every task's block (a Latin square per task),
# and neighbouring tasks in a round start with different arms. Each task block runs under one hold of the desktop lock. After every run it checks
# the STOP file, the API refusal scanner and the pass's list-price spend over every listed arm; past CAP_USD it
# writes STOP-SPEND and ends.
#
# taskset is required: a module with tasks_heldout.py's contract whose folder's FROZEN.md pins every file
# ("`<file>` sha256 `<hex>`", all must match). Every candidate arm (a build under heldout-tools/candidates/) must
# also be pinned there, binary and helper: "arm `<label>` agent-desktop sha256 `<hex>`" and
# "arm `<label>` agent-desktop-macos-helper sha256 `<hex>`". A missing or different pin refuses the pass
# (remote/arms_check.py). rules (v2 by default) is the native audit rule set (HO_RULES); rows record it.
set -u
BT=$BENCH_REAL_HOME/bench-tools
H=$HOME/.t3/worktrees/agent-desktop/perf-speed-cost/bench/heldout
B=$(dirname "$H")
P=$HELDOUT_PY
NAME=$1; ARMS_CSV=$2; TASKS=$3; N=$4; MODEL=$5; TASKSET=$6; export HO_RULES=${7:-v2}; CAP_USD=${CAP_USD:-15}
HOST=${HELDOUT_HOST:-friend-m1pro}
OWNER="perf/speed-cost heldout"
CANDS=$HOME/.agent-desktop-bench-target/heldout-tools/candidates
log() { echo "[$(date +%H:%M:%S)] $*"; }
[[ $N =~ ^[1-9][0-9]?$ ]] || { log "n must be 1-99"; exit 2; }
[[ $HO_RULES =~ ^v[12]$ ]] || { log "rules must be v1 or v2"; exit 2; }
[[ $TASKSET == /* ]] || TASKSET=$HOME/$TASKSET
[ -f "$TASKSET" ] || { log "no task set $TASKSET"; exit 2; }
export HELDOUT_TASKSET=$TASKSET
IFS=, read -r -a ARMS <<< "$ARMS_CSV"
[ ${#ARMS[@]} -ge 1 ] || { log "no arms"; exit 2; }

$P -I "$H/remote/arms_check.py" "$TASKSET" "$CANDS" "$H/arms" "${ARMS[@]}" || { log "refused: see above"; exit 3; }
eval "$($P -I - "$TASKSET" <<'PY'
import importlib.util, sys
spec = importlib.util.spec_from_file_location("heldout_taskset", sys.argv[1])
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
print("ALL_NATIVE=(" + " ".join(m.NATIVE_TASKS) + ")")
print(f"ROWS_DIR=$H/runs/{m.NAME}")
PY
)"
NATIVE_TASKS=()
for t in "${ALL_NATIVE[@]}"; do [[ $TASKS == all || ",$TASKS," == *",$t,"* ]] && NATIVE_TASKS+=("$t"); done
[ ${#NATIVE_TASKS[@]} -gt 0 ] || { log "no native task matches $TASKS"; exit 2; }
NATIVE_ROWS=$ROWS_DIR/results-native.jsonl

rot() { local n=$1; shift; local a=("$@"); local k=$(( n % ${#a[@]} )); echo "${a[@]:$k}" "${a[@]:0:$k}"; }
lines() { [ -f "$1" ] && wc -l < "$1" | tr -d ' ' || echo 0; }
FROM=$(lines "$NATIVE_ROWS")
echo "$NAME $ARMS_CSV $FROM" > "$BT/logs/arms-$NAME.start"
spent() {
  $P -I - "$NATIVE_ROWS" "$FROM" "$B" "${ARMS[@]}" <<'PY'
import json, sys
from pathlib import Path
path, start, arms = Path(sys.argv[1]), int(sys.argv[2]), set(sys.argv[4:])
sys.path.insert(0, sys.argv[3])
import pricing
rows = [json.loads(l) for l in path.read_text().splitlines()[start:] if l.strip()] if path.exists() else []
costs = [pricing.row_cost(r)[0] for r in rows if r.get("arm") in arms]
print(round(sum(float("inf") if c is None else c for c in costs), 4))
PY
}
check() {
  if [ -f "$BT/STOP" ]; then log "STOP file found, ending the pass"; return 0; fi
  if ! $P -I "$BT/tools/api_refusal.py" "$H/runs" "$BT/logs/.since" > "$BT/logs/api_refusal.txt"; then
    log "API REFUSED: $(cat "$BT/logs/api_refusal.txt")"; touch "$BT/STOP-API"; return 0
  fi
  local s; s=$(spent)
  log "$NAME spend so far \$$s (cap \$$CAP_USD)"
  if $P -I -c 'import sys; sys.exit(0 if float(sys.argv[1]) > float(sys.argv[2]) else 1)' "$s" "$CAP_USD"; then
    log "SPEND CAP: $NAME spend \$$s passed \$$CAP_USD"; touch "$BT/STOP-SPEND"; return 0
  fi
  return 1
}
native_run() {
  local arm=$1 task=$2 r=$3
  touch "$BT/logs/.since"
  $P -I "$H/arms/run_native.py" --arm "$arm" --task "$task" --round "$r" --model "$MODEL" --host "$HOST" < /dev/null \
    || { log "native $arm $task exited; retrying once"
         $P -I "$H/arms/run_native.py" --arm "$arm" --task "$task" --round "$r" --model "$MODEL" --host "$HOST" < /dev/null \
           || log "native $arm $task exited again"; }
}

caffeinate -dims -w $$ &
trap 'bash "$B/lockq.sh" release "$OWNER" > /dev/null 2>&1' EXIT
log "ARMS START $NAME arms ${ARMS[*]} model $MODEL rules $HO_RULES rounds 0-$((N - 1)) native ${NATIVE_TASKS[*]}"
for (( r=0; r<N; r++ )); do
  i=0
  for task in $(rot "$r" "${NATIVE_TASKS[@]}"); do
    bash "$B/lockq.sh" take "$OWNER" 5 > /dev/null
    for arm in $(rot $(( r + (r + i) % ${#NATIVE_TASKS[@]} )) "${ARMS[@]}"); do
      native_run "$arm" "$task" "$r"
      if check; then exit 9; fi
    done
    bash "$B/lockq.sh" release "$OWNER" > /dev/null
    i=$(( i + 1 ))
  done
  log "round $r done"
done
log "ARMS COMPLETE $NAME spend \$$(spent)"
