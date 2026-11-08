#!/usr/bin/env bash
# Live probe for the two slowest bench rows. Run under the bench lock.
#
# Finder: after New Folder the name is in an inline editor. Which delivery
# commits a new name, and does any agent-desktop call end the edit first?
# Each variant makes a fresh folder with Finder frontmost, writes the name, and
# checks the folder name on disk, independently of agent-desktop.
#
# TextEdit: the Save sheet against its document window. Dumps both frames and
# tries Cancel (harmless) with the base and head binaries, so an offscreen
# refusal caused by the sheet being wider than its window shows up directly.
#
# Usage: bash bench/probe_inline_edit.sh <base binary> <head binary>
set -u
cd "$(dirname "$0")/.."
BASE="${1:?base binary}"; HEAD="${2:?head binary}"
M=bench/out/macctl
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
out=bench/out/probe_inline_edit; mkdir -p "$out"
log="$out/results-$(date +%H%M%S).txt"
D="$HOME/agent-desktop-bench/probe-inline"
rm -rf "$D"; mkdir -p "$D"

new_folder() {
    $M activate Finder; $M raise Finder probe-inline 2>/dev/null; sleep 0.5
    $M key Finder 45 cmd,shift; sleep 1.5
}
on_disk() { ls "$D" | tr '\n' '|'; }

open "$D"; sleep 3
for variant in control-macctl focus-after-status focus-after-list-windows ad-press ad-press-headed ad-type-focused; do
    name="n-$variant"
    new_folder
    before=$($M focused Finder)
    case "$variant" in
        control-macctl) $M set-focused Finder "$name"; $M key Finder 36 ;;
        focus-after-status) $BASE status >/dev/null; mid=$($M focused Finder); $M set-focused Finder "$name"; $M key Finder 36 ;;
        focus-after-list-windows) $BASE list-windows --app Finder >/dev/null; mid=$($M focused Finder); $M set-focused Finder "$name"; $M key Finder 36 ;;
        ad-press) $M set-focused Finder "$name"; res=$($BASE press return --app Finder) ;;
        ad-press-headed) $M set-focused Finder "$name"; res=$($BASE --headed press return --app Finder) ;;
        ad-type-focused)
            ref=$($BASE find --app Finder --role textfield --value "untitled folder" --exact 2>/dev/null | python3 -c "
import json,sys
m=json.load(sys.stdin).get('data',{}).get('matches',[])
print(m[0]['ref_id'] if m else '')")
            res=$($BASE type "$ref" "$name" 2>&1); $M key Finder 36 ;;
    esac
    sleep 1.5
    printf '%-26s before=%s mid=%s disk=[%s] res=%s\n' "$variant" "$before" "${mid:-}" "$(on_disk)" "$(echo "${res:-}" | head -c 300)" | tee -a "$log"
    unset mid res
    $M key Finder 53; sleep 0.5
done
$M close-discard Finder probe-inline

open -a TextEdit; sleep 2
$M activate TextEdit; $M key TextEdit 45 cmd; sleep 1.5
$M key TextEdit 1 cmd; sleep 2
for side in base head; do
    B=$BASE; [ "$side" = head ] && B=$HEAD
    $B snapshot --app TextEdit --surface sheet > "$out/sheet-$side.json" 2>&1
    python3 - "$out/sheet-$side.json" <<'PY' | tee -a "$log"
import json, sys
d = json.load(open(sys.argv[1]))
tree = d.get("data", {}).get("tree") or {}
print("sheet frame", tree.get("bounds"), "window", d.get("data", {}).get("window", {}).get("bounds"))
def walk(n):
    yield n
    for c in n.get("children") or []: yield from walk(c)
for n in walk(tree):
    if n.get("role") == "button" and n.get("name") in ("Cancel", "Save"):
        print(" ", n.get("name"), n.get("ref_id"), n.get("bounds"), n.get("states"))
PY
    ref=$(python3 -c "
import json
d=json.load(open('$out/sheet-$side.json'))
def walk(n):
    yield n
    for c in n.get('children') or []: yield from walk(c)
print(next((n['ref_id'] for n in walk(d.get('data',{}).get('tree') or {}) if n.get('role')=='button' and n.get('name')=='Cancel'), ''))")
    res=$($B click "$ref" 2>&1)
    echo "$side click Cancel $ref: $(echo "$res" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d.get("ok"), (d.get("error") or {}).get("message"), (d.get("data") or {}).get("disposition"))' 2>&1)" | tee -a "$log"
    sleep 1
    $B list-surfaces --app TextEdit | python3 -c 'import json,sys; print("  surfaces after:", [s.get("kind") for s in json.load(sys.stdin).get("data",{}).get("surfaces",[])])' | tee -a "$log"
    [ "$side" = base ] && { $M key TextEdit 1 cmd; sleep 2; }
done
$M close-discard TextEdit Untitled
echo "done $log"
