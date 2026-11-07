#!/usr/bin/env bash
# Live probe: which input path commits a Finder rename. For each variant it
# makes a new folder (cmd+shift+n), writes a name into the rename field, presses
# Return, and checks the folder name on disk, independently of agent-desktop.
# Usage: bash bench/probe_rename.sh [binary]. Run under the bench lock.
set -u
cd "$(dirname "$0")/.."
B="${1:-bench/out/wt-base/target/release/agent-desktop}"
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
D="$HOME/agent-desktop-bench/probe-rename"
mkdir -p "$D"
for d in "$D"/*; do [ -d "$d" ] && rmdir "$d" 2>/dev/null; done
open "$D"; sleep 3
osascript -e 'tell application "Finder" to set current view of (first Finder window whose name is "probe-rename") to list view' >/dev/null 2>&1
W=$($B list-windows --app Finder | python3 -c "
import json,subprocess,sys
for w in json.load(sys.stdin)['data']:
    s=json.loads(subprocess.run(['$B','snapshot','--window-id',w['id'],'--max-depth','1'],capture_output=True,text=True).stdout)
    if s.get('data',{}).get('tree',{}).get('name')=='probe-rename': print(w['id']); break")
echo "window $W"
field() {
    $B find --window-id "$W" --role textfield --timeout-ms 15000 | python3 -c "
import json,sys
for m in json.load(sys.stdin).get('data',{}).get('matches',[]):
    if (m.get('value') or '').startswith('untitled folder'): print(m['ref_id']); break"
}
for variant in set-value type type-headed set-value-headed-return; do
    name="renamed-$variant"
    osascript -e 'tell application "Finder" to activate' -e 'tell application "Finder" to set index of (first Finder window whose name is "probe-rename") to 1' >/dev/null 2>&1
    sleep 0.5
    $B press cmd+shift+n --app Finder >/dev/null
    sleep 1.5
    ref=$(field)
    [ -n "$ref" ] || { echo "$variant: no rename field"; continue; }
    case "$variant" in
        set-value) out=$($B set-value "$ref" "$name") ;;
        type) out=$($B type "$ref" "$name") ;;
        type-headed) out=$($B --headed type "$ref" "$name") ;;
        set-value-headed-return) out=$($B set-value "$ref" "$name") ;;
    esac
    case "$variant" in
        *headed*) $B --headed press return --app Finder >/dev/null ;;
        *) $B press return --app Finder >/dev/null ;;
    esac
    sleep 1.5
    on_disk=$(ls "$D" | tr '\n' ',')
    printf "%-24s ok=%s delivery=%s on_disk=[%s]\n" "$variant" \
        "$(echo "$out" | grep -o '"ok":[a-z]*' | head -1)" "$(echo "$out" | grep -o '"delivery":"[a-z_]*"' | head -1)" "$on_disk"
done 2>&1 | tee bench/out/probe_rename.txt
