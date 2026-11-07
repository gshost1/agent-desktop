#!/usr/bin/env bash
# Cold-start profile: time launch and the first snapshot of a freshly started
# app, with and without a settle pause, and log whether renderer activation ran.
# Usage: bash bench/cold_start.sh [app ...]
set -u
cd "$(dirname "$0")/.."
B=target/release/agent-desktop
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-cold"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
now() { python3 -c 'import time;print(time.time())'; }
for app in "${@:-Weather Calculator TextEdit}"; do
  for pause in 0 5; do
    for i in 1 2 3; do
      osascript -e "if application \"$app\" is running then tell application \"$app\" to quit saving no" >/dev/null 2>&1 || osascript -e "if application \"$app\" is running then tell application \"$app\" to quit" >/dev/null 2>&1
      sleep 2
      t0=$(now); $B launch "$app" --activate >/dev/null; t1=$(now)
      sleep "$pause"
      t2=$(now); $B -v snapshot --app "$app" -i --compact 2>bench/out/cold.err >/dev/null; t3=$(now)
      t4=$(now); $B snapshot --app "$app" -i --compact >/dev/null; t5=$(now)
      act=$(grep -ciE "activat|enhanced" bench/out/cold.err)
      printf "%-12s pause=%s run=%s launch=%.2fs first_snapshot=%.2fs second_snapshot=%.2fs activation_log_lines=%s\n" "$app" "$pause" "$i" "$(echo "$t1-$t0"|bc)" "$(echo "$t3-$t2"|bc)" "$(echo "$t5-$t4"|bc)" "$act"
    done
  done
done
