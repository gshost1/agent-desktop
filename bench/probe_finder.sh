#!/usr/bin/env bash
# Live probe: why Finder windows and file panels are slow to observe and act on.
# Opens a Finder window on a bench folder holding many items, then times
# snapshot/find with verbose locator stats. Read-only except for the window it opens.
# Usage: bash bench/probe_finder.sh
set -u
cd "$(dirname "$0")/.."
B=target/release/agent-desktop
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
D="$HOME/agent-desktop-bench/probe-many"
mkdir -p "$D"; for i in $(seq 1 60); do mkdir -p "$D/folder-$i"; done
osascript -e "tell application \"Finder\" to open POSIX file \"$D\"" >/dev/null
sleep 2
win_named() { for w in $($B list-windows --app Finder | python3 -c "import json,sys; [print(w['id']) for w in json.load(sys.stdin)['data']]"); do n=$($B snapshot --window-id "$w" --max-depth 1 | python3 -c "import json,sys; print(json.load(sys.stdin)['data']['tree'].get('name',''))" 2>/dev/null); [ "$n" = "$1" ] && { echo "$w"; return; }; done; }
W=$(win_named probe-many)
echo "window $W"
stats() { grep -o "nodes_visited: [0-9]*\|attribute_batches: [0-9]*\|child_reads: [0-9]*\|action_reads: [0-9]*\|settable_reads: [0-9]*\|child_label_reads: [0-9]*\|promotion_reads: [0-9]*" | tr '\n' ' '; }
for args in "snapshot -i --outline" "snapshot -i" "snapshot -i --timeout-ms 20000" "snapshot --skeleton -i" "find --role button --first" "find --text folder-59"; do
  t0=$(python3 -c 'import time;print(time.time())')
  bin=$B; case "$args" in *--outline*) bin=bench/out/wt-outline/target/release/agent-desktop ;; esac
  out=$($bin -v $args --window-id "$W" 2>bench/out/probe.err)
  t1=$(python3 -c 'import time;print(time.time())')
  printf "%-36s %6.2fs %7dB %s | %s\n" "$args" "$(echo "$t1-$t0"|bc)" "${#out}" "$(echo "$out" | grep -o '"complete":[a-z]*\|"truncated":[a-z]*\|"nodes_observed":[0-9]*\|"code":"[A-Z_]*"' | tr '\n' ' ')" "$(stats < bench/out/probe.err)"
done

echo "== click-on-row probe (ADAPTERS-2): does one headless click select only, or also open?"
for view in "list view" "icon view"; do
  osascript -e "tell application \"Finder\" to set target of (first Finder window whose name is \"probe-many\" or name is \"folder-7\") to POSIX file \"$D\"" >/dev/null 2>&1
  osascript -e "tell application \"Finder\" to set current view of (first Finder window whose name is \"probe-many\") to $view" >/dev/null
  sleep 1.5
  W=$(win_named probe-many)
  snap=$($B snapshot --window-id "$W" -i --timeout-ms 20000)
  ref=$(echo "$snap" | python3 -c "
import json,sys
d=json.load(sys.stdin)
def walk(n):
    yield n
    for c in n.get('children',[]): yield from walk(c)
for n in walk(d['data']['tree']):
    if n.get('ref_id') and 'folder-7' == (n.get('name') or n.get('value') or ''):
        print(n['ref_id'], n.get('role')); break
")
  echo "[$view] target: $ref"
  r=${ref%% *}
  [ -n "$r" ] || { echo "  no ref for folder-7"; continue; }
  before=$(osascript -e 'tell application "Finder" to get name of every Finder window')
  out=$($B -v click "$r" 2>bench/out/probe-click.err)
  sleep 1
  after=$(osascript -e 'tell application "Finder" to get name of every Finder window')
  sel=$(osascript -e "tell application \"Finder\" to get name of selection")
  echo "  click: $(echo "$out" | head -c 400)"
  echo "  steps(debug): $(grep -oE "AXPress|AXOpen|DeliveredWithoutEffect|press_selection|selection|child press|AXConfirm" bench/out/probe-click.err | sort | uniq -c | tr '\n' ' ')"
  echo "  windows before: $before"
  echo "  windows after:  $after"
  echo "  finder selection: $sel"
done
