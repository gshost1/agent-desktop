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
bench/out/macctl raise Finder probe-rename; bench/out/macctl menu Finder View "as List" 2>/dev/null
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
mkdir -p bench/out/probe_rename
for variant in warmup set-value type set-value-headed-return press-keys; do
    name="renamed-$variant"
    bench/out/macctl raise Finder probe-rename
    sleep 0.5
    $B press cmd+shift+n --app Finder >/dev/null
    sleep 1.5
    ref=$(field)
    [ -n "$ref" ] || { sleep 2; ref=$(field); }
    [ -n "$ref" ] || { echo "$variant: no rename field"; continue; }
    case "$variant" in
        warmup) $B press escape --app Finder >/dev/null; continue ;;
        set-value) out=$($B set-value "$ref" "$name") ;;
        type) out=$($B type "$ref" "$name") ;;
        type-headed) out=$($B --headed type "$ref" "$name") ;;
        type-interval-headed) out=$($B --headed type "$ref" "$name" --interval-ms 20) ;;
        press-keys) out=""; for ch in $(echo "$name" | grep -o .); do k=$ch; [ "$ch" = "-" ] && k=minus; out="$out$($B press "$k" --app Finder)"; done ;;
        set-value-headed-return) out=$($B set-value "$ref" "$name") ;;
    esac
    case "$variant" in
        *headed*) $B --headed press return --app Finder >/dev/null ;;
        *) $B press return --app Finder >/dev/null ;;
    esac
    echo "$out" > "bench/out/probe_rename/$variant.json"
    $B get "$ref" --property value > "bench/out/probe_rename/$variant.readback.json" 2>&1
    sleep 1.5
    on_disk=$(ls "$D" | tr '\n' ',')
    printf "%-24s ok=%s delivery=%s on_disk=[%s]\n" "$variant" \
        "$(echo "$out" | grep -o '"ok":[a-z]*' | head -1)" "$(echo "$out" | grep -o '"delivery":"[a-z_]*"' | head -1)" "$on_disk"
done 2>&1 | tee bench/out/probe_rename.txt
# v4: rename through the item's Info window, the route a passing model run used.
# The inline field stays a negative control above; here the Name field of the
# Info window gets set-value, then Return goes to that window.
{
for attempt in 1 2; do
    name="renamed-info-$attempt"
    bench/out/macctl raise Finder probe-rename
    $B press cmd+shift+n --app Finder >/dev/null; sleep 1.5
    $B press escape --app Finder >/dev/null; sleep 0.5
    before=$(ls "$D" | tr '\n' ',')
    item=$(ls -t "$D" | head -1)
    menu=$($B find --app Finder --surface menubar --role menuitem --name "Get Info" --exact | python3 -c "
import json,sys
m=json.load(sys.stdin).get('data',{}).get('matches',[])
print(m[0]['ref_id'] if m else '')")
    [ -n "$menu" ] || { echo "get-info-$attempt: no Get Info menu item"; continue; }
    $B click "$menu" > "bench/out/probe_rename/get-info-$attempt.menu.json"; sleep 1.5
    info=$($B list-windows --app Finder | python3 -c "
import json,sys
for w in json.load(sys.stdin).get('data',[]):
    if (w.get('title') or '').endswith(' Info'): print(w['id'], w['title']); break")
    wid=${info%% *}
    [ -n "$wid" ] || { echo "get-info-$attempt: no Info window (selected item $item)"; continue; }
    field=$($B find --window-id "$wid" --role textfield --value "$item" --exact | python3 -c "
import json,sys
m=json.load(sys.stdin).get('data',{}).get('matches',[])
print(m[0]['ref_id'] if m else '')")
    [ -n "$field" ] || { echo "get-info-$attempt: no Name field holding '$item' in ${info#* }"; continue; }
    out=$($B set-value "$field" "$name")
    $B press return --window-id "$wid" >/dev/null; sleep 1.5
    echo "$out" > "bench/out/probe_rename/get-info-$attempt.json"
    on_disk=$(ls "$D" | tr '\n' ',')
    printf "%-24s ok=%s delivery=%s renamed=%s on_disk=[%s]\n" "get-info-$attempt" \
        "$(echo "$out" | grep -o '"ok":[a-z]*' | head -1)" "$(echo "$out" | grep -o '"delivery":"[a-z_]*"' | head -1)" \
        "$([ -d "$D/$name" ] && echo yes || echo no)" "$on_disk"
    bench/out/macctl close-discard Finder "${info#* }"
done
} 2>&1 | tee -a bench/out/probe_rename.txt
