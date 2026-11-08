#!/usr/bin/env bash
# Live check for the find menu-bar fallback, base (fork main) against head:
# a --role menuitem find per app, then the menu bar snapshot it replaces.
# Records ok, surface, match count, time and bytes per call.
cd "$(dirname "$0")/.."
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
base=bench/out/wt-main/target/release/agent-desktop
head=bench/out/wt-findmenu/target/release/agent-desktop
out=bench/out/check_find_menu.txt; : > "$out"
call() {
    local label="$1"; shift
    local t0 t1 body
    t0=$(python3 -c 'import time; print(time.time())')
    body=$("$@" 2>/dev/null)
    t1=$(python3 -c 'import time; print(time.time())')
    printf '%s\t%s\n' "$label" "$(printf '%s' "$body" | python3 -c '
import json, sys
raw = sys.stdin.read(); t0, t1 = float(sys.argv[1]), float(sys.argv[2])
try: env = json.loads(raw)
except ValueError: print("unparsed", len(raw)); sys.exit()
d = env.get("data") or {}
n = len(d.get("matches") or []) if "matches" in d else (1 if d.get("match") else 0)
print(f"ok={env.get(\"ok\")} code={(env.get(\"error\") or {}).get(\"code\")} surface={d.get(\"surface\")} matches={n} ms={round((t1-t0)*1000)} bytes={len(raw)}")
' "$t0" "$t1")" | tee -a "$out"
}
bench/out/macctl activate Finder >/dev/null 2>&1
for app_item in "Finder:New Folder" "Finder:Go to Folder" "TextEdit:Save" "Calculator:Scientific"; do
    app="${app_item%%:*}"; item="${app_item#*:}"
    "$head" launch "$app" >/dev/null 2>&1; sleep 1
    for rep in 1 2 3; do
        for side in base head; do
            bin=$base; [ "$side" = head ] && bin=$head
            call "$side $app '$item' find#$rep" "$bin" find --app "$app" --role menuitem --name "$item"
        done
    done
    call "base $app menubar-snapshot" "$base" snapshot --app "$app" --surface menubar -i --compact
done
bench/out/macctl quit Calculator >/dev/null 2>&1
bench/out/macctl close-discard TextEdit Untitled >/dev/null 2>&1
bench/out/macctl quit TextEdit >/dev/null 2>&1
bash bench/run_perf.sh wt-findmenu | tee -a "$out"
