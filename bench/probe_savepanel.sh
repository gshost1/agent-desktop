#!/usr/bin/env bash
# Opens a TextEdit save sheet and records why ref actions in it wait out the
# actionability budget: raw AX reads per control, then a click on the Where:
# pop-up through base and head binaries with a debug trace.
set -u
cd "$(dirname "$0")/.."
out=bench/out/probe_savepanel; mkdir -p "$out"
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
B="${1:-bench/out/wt-base/target/release/agent-desktop}"
H="${2:-bench/out/wt-popup/target/release/agent-desktop}"
[ bench/ax_sheet_attrs.swift -nt bench/out/ax_sheet_attrs ] && swiftc -O bench/ax_sheet_attrs.swift -o bench/out/ax_sheet_attrs
"$B" launch TextEdit --activate >/dev/null
sleep 1
"$B" press cmd+n --app TextEdit >/dev/null; sleep 1
"$B" press cmd+s --app TextEdit >/dev/null; sleep 2
pid=$("$B" list-apps | python3 -c 'import json,sys; print(next(a["pid"] for a in json.load(sys.stdin)["data"] if a["name"]=="TextEdit"))')
bench/out/ax_sheet_attrs "$pid" > "$out/ax_attrs.jsonl" 2>&1
pick_ref() {
python3 - "$1" <<'PY'
import json,sys
t=json.load(open(sys.argv[1]))["data"]["tree"]
def walk(n):
    yield n
    for c in n.get("children",[]): yield from walk(c)
for n in walk(t):
    if n.get("role") in ("combobox","popupbutton") and n.get("ref_id"):
        print(n["ref_id"]); break
PY
}
: > "$out/click.txt"
for label in base head; do
    bin=$B; [ $label = head ] && bin=$H
    "$bin" snapshot --app TextEdit --surface sheet -i --compact > "$out/sheet-$label.json"
    ref=$(pick_ref "$out/sheet-$label.json")
    if [ -z "$ref" ]; then
        "$B" press cmd+s --app TextEdit >/dev/null; sleep 2
        "$bin" snapshot --app TextEdit --surface sheet -i --compact > "$out/sheet-$label.json"
        ref=$(pick_ref "$out/sheet-$label.json")
    fi
    s=$(python3 -c 'import time; print(time.time())')
    RUST_LOG=agent_desktop=debug,agent_desktop_core=debug,agent_desktop_macos=debug "$bin" click "$ref" > "$out/click-$label.json" 2> "$out/click-$label.log"
    e=$(python3 -c 'import time; print(time.time())')
    echo "$label click $ref ms $(python3 -c "print(round(($e-$s)*1000))") $(head -c 300 "$out/click-$label.json")" >> "$out/click.txt"
    "$B" press escape --app TextEdit >/dev/null; sleep 1
done
"$B" press escape --app TextEdit >/dev/null; sleep 1
osascript -e 'tell application "TextEdit" to close front document saving no' >/dev/null 2>&1
cat "$out/click.txt"; head -40 "$out/ax_attrs.jsonl"
