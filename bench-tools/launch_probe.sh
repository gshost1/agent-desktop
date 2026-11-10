#!/usr/bin/env bash
# usage: launch_probe.sh <binary> <runs> <app>...   prints per-run timings, no app content
set -u
B="$1"; N="$2"; shift 2
MC=/Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/out/macctl
WW=/private/tmp/claude-501/-Users-Shost2--t3-worktrees-agent-desktop-t3-7c029123/6c037495-151e-453b-a8c7-33efd49f72b5/scratchpad/probe/winwatch
export AGENT_DESKTOP_HOME=$(mktemp -d /private/tmp/claude-501/-Users-Shost2--t3-worktrees-agent-desktop-t3-7c029123/6c037495-151e-453b-a8c7-33efd49f72b5/scratchpad/home.XXXX); chmod 700 "$AGENT_DESKTOP_HOME"
now() { python3 -c 'import time;print(time.time())'; }
for app in "$@"; do
  for i in $(seq 0 "$N"); do
    "$MC" quit "$app" >/dev/null 2>&1; sleep 2
    case "$app" in Weather) bid=com.apple.weather;; Calculator) bid=com.apple.calculator;; *) bid=com.apple.systempreferences;; esac; "$WW" "$bid" 8 > "$AGENT_DESKTOP_HOME/ww.txt" & wpid=$!
    t0=$(now); "$B" launch "$app" >/dev/null; t1=$(now)
    if [ "$app" = "System Settings" ]; then "$B" find --app "$app" --name General | jq -r .ok > "$AGENT_DESKTOP_HOME/ok1"; else "$B" snapshot --app "$app" -i --compact --lines | jq -r .ok > "$AGENT_DESKTOP_HOME/ok1"; fi; t2=$(now)
    "$B" snapshot --app "$app" -i --compact --lines >/dev/null; t3=$(now)
    wait $wpid
    printf "%-16s run=%s launch=%.2f first_obs=%.2f warm_snap=%.2f %s\n" "$app" "$i" "$(echo "$t1-$t0"|bc)" "$(echo "$t2-$t1"|bc)" "$(echo "$t3-$t2"|bc)" "ok=$(cat $AGENT_DESKTOP_HOME/ok1) $(cat $AGENT_DESKTOP_HOME/ww.txt)"
  done
  "$MC" quit "$app" >/dev/null 2>&1
done
rm -rf "$AGENT_DESKTOP_HOME"
