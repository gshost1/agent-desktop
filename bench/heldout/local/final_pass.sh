#!/bin/bash
# Started by final_local.sh, detached: keeps the Mac awake for the whole pass, logs a watchdog line every 5 minutes,
# runs the model stub for a dry run, and runs remote/arms_pass.sh with its saved schedule (all tasks, the schedule's
# rounds).
#   final_pass.sh <name> <arms,csv> <taskset> <model> <rules>
set -u
H=$(cd "$(dirname "$0")/.." && pwd)
NAME=$1; ARMS_CSV=$2; TASKSET=$3; MODEL=$4; RULES=$5
BT=$BENCH_REAL_HOME/bench-tools
log() { echo "[$(date +%H:%M:%S)] $*"; }
caffeinate -dims -w $$ &
pids=()
if [ -n "${HO_MODEL_STUB_CONFIG:-}" ]; then
  "$HELDOUT_PY" -I "$H/local/model_stub.py" "$HO_MODEL_STUB_PORT" "$HO_MODEL_STUB_CONFIG" &
  pids+=($!)
  log "DRY RUN: model stub on 127.0.0.1:$HO_MODEL_STUB_PORT, no model call leaves the Mac"
fi
rows_dir=$("$HELDOUT_PY" -I - "$TASKSET" <<'PY'
import importlib.util, sys
from pathlib import Path
spec = importlib.util.spec_from_file_location("heldout_taskset", sys.argv[1])
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
print(m.NAME)
PY
)
watchdog() {
  while sleep 300; do
    "$HELDOUT_PY" -I - "$H/runs/$rows_dir" "$NAME" "$H/.." <<'PY'
import json, os, sys, time
from pathlib import Path
runs, name = Path(sys.argv[1]), sys.argv[2]
sys.path.insert(0, sys.argv[3])
import pricing
def read(p):
    return [json.loads(l) for l in p.read_text().splitlines() if l.strip()] if p.exists() else []
rows = [r for r in read(runs / "results-native.jsonl") if r.get("experiment") == name]
led = [r for r in read(runs / "attempts.jsonl") if r.get("experiment") == name]
costs = [pricing.row_cost(r)[0] for r in rows]
spend = sum(float("inf") if c is None else c for c in costs)
last = max((r.get("t", 0) for r in led), default=0)
load = os.getloadavg()[0]
print(f"[{time.strftime('%H:%M:%S')}] WATCHDOG alive: {len(rows)} rows, {sum(r['state'] == 'started' for r in led)} "
      f"started, spend ${spend:.2f}, last ledger line {time.time() - last:.0f} s ago, load {load:.1f}", flush=True)
PY
  done
}
watchdog &
pids+=($!)
trap 'kill "${pids[@]}" 2>/dev/null' EXIT
log "LOCAL PASS $NAME arms $ARMS_CSV model $MODEL rules $RULES cap \$$CAP_USD host $HELDOUT_HOST"
/bin/bash "$H/remote/arms_pass.sh" "$NAME" "$ARMS_CSV" all 1 "$MODEL" "$TASKSET" "$RULES"
rc=$?
log "LOCAL PASS $NAME ended, arms_pass exit $rc"
exit $rc
