#!/bin/bash
# Run the GUI matrix one round per lock hold, then yield the lock to the other session's next round.
# Usage: rounds.sh FIRST LAST -- <run.py args without --runs/--start-run>
# POST_ROUND_CMD (optional) runs after each round while the lock is still held, then the lock is released.
first=$1; last=$2; shift 3
here="$(cd "$(dirname "$0")" && pwd)"
lock="$HOME/.agent-desktop/bench.lock"
for r in $(seq "$first" "$last"); do
  python3 "$here/run.py" --runs 1 --start-run "$r" --keep-lock "$@" || echo "round $r exited $?"
  if [ -n "$POST_ROUND_CMD" ] && grep -q "feat/unified-browser bench" "$lock" 2>/dev/null; then
    echo "post-round $r: $POST_ROUND_CMD"; bash -c "$POST_ROUND_CMD" || echo "post-round exited $?"
  fi
  python3 "$here/lock.py" release "feat/unified-browser bench"
  echo "round $r released $(date -u +%T)"
  [ "$r" -lt "$last" ] || break
  for _ in $(seq 1 180); do
    if [ -f "$lock" ] && ! grep -q "feat/unified-browser" "$lock"; then break; fi
    sleep 20
  done
done
