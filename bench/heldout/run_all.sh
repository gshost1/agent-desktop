#!/bin/bash
# Held-out benchmark driver: rounds FROM..TO, every task once per arm per round, arm and task order
# rotated each round. Native arms run one arm per desktop-lock hold; each web arm's round is one
# run.py call, which takes and releases the lock itself.
#
#   bash heldout/run_all.sh <model> <from_round> <to_round> [all|native|web]
#   NATIVE_ARMS="ad-14 ad-up peekaboo cua" WEB_ARMS="ad-14 ad-up bh pw" to choose arms (drop order: pw, peekaboo).
#
# Refuses to start when tasks_heldout.py does not match the sha256 frozen in TASKS_FROZEN.md.
set -u
H=$(cd "$(dirname "$0")" && pwd)
B=$(dirname "$H")
P=${HELDOUT_PY:-$HOME/.local/bin/python3.11}
MODEL=${1:-claude-opus-5-5}; FROM=${2:-0}; TO=${3:-4}; SUITE=${4:-all}
read -r -a NATIVE_ARMS <<< "${NATIVE_ARMS:-ad-14 ad-up peekaboo cua}"
read -r -a WEB_ARMS <<< "${WEB_ARMS:-ad-14 ad-up bh pw}"
NATIVE_TASKS=(hn1 hn2 hn3 hn4 hn5 hn6 hn7 hn8)
WEB_TASKS=(hw1 hw2 hw3 hw4 hw5 hw6 hw7 hp1 hp2 hp3)
OWNER="perf/speed-cost heldout"
HOST=${HELDOUT_HOST:-mac}
SR_FLAG=${HELDOUT_SCREEN_RECORDING:+--screen-recording}

frozen=$(grep -oE 'tasks_heldout.py sha256 `[0-9a-f]{64}`' "$H/TASKS_FROZEN.md" | grep -oE '[0-9a-f]{64}')
actual=$(shasum -a 256 "$H/tasks_heldout.py" | cut -d' ' -f1)
[ -n "$frozen" ] && [ "$frozen" = "$actual" ] || { echo "tasks_heldout.py sha256 $actual does not match the frozen $frozen"; exit 3; }

rot() { local n=$1; shift; local a=("$@"); local k=$(( n % ${#a[@]} )); echo "${a[@]:$k}" "${a[@]:0:$k}"; }
log() { echo "[$(date +%H:%M:%S)] $*"; }

server_pid=""
if [ "$SUITE" != native ]; then
  if ! $P -I -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8792/__state', timeout=3)" 2>/dev/null; then
    $P -I "$H/web/server.py" 8792 > "$H/out/server.log" 2>&1 &
    server_pid=$!
    sleep 1
  fi
fi
trap '[ -n "$server_pid" ] && kill "$server_pid" 2>/dev/null' EXIT

for (( r=FROM; r<=TO; r++ )); do
  if [ "$SUITE" != web ]; then
    for arm in $(rot "$r" "${NATIVE_ARMS[@]}"); do
      bash "$B/lockq.sh" take "$OWNER" 5 > /dev/null
      log "round $r native $arm start"
      for task in $(rot "$r" "${NATIVE_TASKS[@]}"); do
        $P -I "$H/arms/run_native.py" --arm "$arm" --task "$task" --round "$r" --model "$MODEL" --host "$HOST" $SR_FLAG < /dev/null \
          || log "native $arm $task exited $?"
      done
      [ "$arm" = cua ] && "$H/arms/bin/cua/cua-driver" stop > /dev/null 2>&1
      bash "$B/lockq.sh" release "$OWNER" > /dev/null
      log "round $r native $arm done"
    done
  fi
  if [ "$SUITE" != native ]; then
    tasks=$(rot "$r" "${WEB_TASKS[@]}" | tr ' ' ',')
    for arm in $(rot "$r" "${WEB_ARMS[@]}"); do
      log "round $r web $arm start"
      $P -I "$H/arms/run_web.py" --arm "$arm" --tasks "$tasks" --runs 1 --start-run "$r" --model "$MODEL" --host "$HOST" \
        --timeout 600 < /dev/null || log "web $arm round $r exited $?"
      log "round $r web $arm done"
    done
  fi
done
log "HELDOUT COMPLETE $MODEL rounds $FROM-$TO $SUITE"
