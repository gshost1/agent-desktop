#!/usr/bin/env bash
# Why the E2E headless gate sees no "one unambiguous owned window": launch the
# test fixture the way tests/e2e/safe-semantic.sh does (no activation) and record
# what list-windows reports for it over 4 s, beside the Accessibility view.
cd "$(dirname "$0")/.."
B=bench/out/wt-surface/target/release/agent-desktop
F=bench/out/perf-wt-cold/fixture/AgentDeskFixture.app/Contents/MacOS/AgentDeskFixture
out=bench/out/e2e_fixture_probe; mkdir -p "$out"
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
bench/out/macctl front > "$out/front-before.json"
AGENT_DESKTOP_FIXTURE_NO_ACTIVATE=1 "$F" > "$out/fixture.stdout" 2> "$out/fixture.stderr" &
pid=$!
echo "fixture pid $pid" > "$out/summary.txt"
for t in 0.5 1 2 4; do
    sleep "$t"
    "$B" list-windows --app AgentDeskFixture > "$out/list-windows-app-$t.json" 2>&1
    "$B" list-windows > "$out/list-windows-all-$t.json" 2>&1
    python3 - "$out/list-windows-app-$t.json" "$out/list-windows-all-$t.json" "$pid" "$t" >> "$out/summary.txt" <<'PY'
import json, sys
app, everything, pid, t = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
def load(p):
    try: return json.load(open(p))
    except Exception as e: return {"unparsed": open(p).read()[:300]}
a, e = load(app), load(everything)
mine = [w for w in (e.get("data") or []) if w.get("pid") == pid]
print(t, "app-filter ok", a.get("ok"), "count", len(a.get("data") or []), "error", (a.get("error") or {}).get("code"))
for w in (a.get("data") or []) + mine:
    print("   ", {k: w.get(k) for k in ("app_name", "pid", "title", "visible", "minimized", "is_focused", "id")})
PY
done
bench/out/macctl windows AgentDeskFixture > "$out/ax-windows.json" 2>&1
bench/out/macctl front > "$out/front-after.json"
kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
cat "$out/summary.txt" "$out/ax-windows.json"
