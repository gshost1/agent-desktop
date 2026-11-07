#!/usr/bin/env bash
# Live A/B of action latency between two agent-desktop binaries.
# Finder: headless click on a list-view row of a 60-folder window, verified by
# Finder's own selection (AppleScript to Finder). Calculator: headless click on
# a digit button, verified by a display read through the other binary.
# Seven measured runs per case after one warm-up; prints min/median/max.
# Usage: bash bench/ab_click.sh <base-binary> <head-binary> [out.json]
set -u
cd "$(dirname "$0")/.."
base="$1"; head="$2"; out="${3:-bench/out/ab_click.json}"
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-ab"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
D="$HOME/agent-desktop-bench/probe-many"
mkdir -p "$D"; for i in $(seq 1 60); do mkdir -p "$D/folder-$i"; done
for attempt in 1 2 3 4 5; do
    open "$D"
    sleep 3
    osascript -e "tell application \"Finder\" to set current view of (first Finder window whose name is \"probe-many\") to list view" >/dev/null 2>&1 && break
done
open -g -a Calculator
sleep 3
python3 - "$base" "$head" "$out" <<'EOF'
import json, statistics, subprocess, sys, time
base, head, out = sys.argv[1:4]

def ad(binary, *args):
    t0 = time.perf_counter()
    p = subprocess.run([binary, *args], capture_output=True, text=True, timeout=120)
    ms = (time.perf_counter() - t0) * 1000
    try:
        return ms, json.loads(p.stdout)
    except ValueError:
        return ms, {}

def walk(n, anc=()):
    yield n, anc
    for c in n.get("children", []):
        yield from walk(c, anc + (n,))

def finder_window(binary):
    _, w = ad(binary, "list-windows", "--app", "Finder")
    for win in w.get("data", []):
        _, s = ad(binary, "snapshot", "--window-id", win["id"], "--max-depth", "1")
        if s.get("data", {}).get("tree", {}).get("name") == "probe-many":
            return win["id"]
    raise SystemExit("probe-many window not found")

def finder_row(binary, window, name):
    _, s = ad(binary, "snapshot", "--window-id", window, "-i", "--timeout-ms", "20000")
    for n, anc in walk(s["data"]["tree"]):
        if (n.get("name") or n.get("value")) == name:
            rows = [a for a in anc if a.get("role") == "treeitem" and a.get("ref_id")]
            if rows:
                return rows[-1]["ref_id"]
    raise SystemExit(f"no row for {name}")

def finder_selection():
    p = subprocess.run(["osascript", "-e", 'tell application "Finder" to get name of item 1 of (selection as list)'],
                       capture_output=True, text=True, timeout=30)
    return p.stdout.strip()

def calc_button(binary, label):
    _, s = ad(binary, "snapshot", "--app", "Calculator", "-i")
    for n, _ in walk(s["data"]["tree"]):
        if n.get("role") == "button" and n.get("name") == label and n.get("ref_id"):
            return n["ref_id"]
    raise SystemExit(f"no Calculator button {label}")

def calc_display(binary):
    _, s = ad(binary, "snapshot", "--app", "Calculator")
    for n, _ in walk(s.get("data", {}).get("tree", {})):
        if n.get("role") == "scrollarea" and n.get("name") == "Edit field":
            texts = [m.get("value") or m.get("name") for m, _ in walk(n) if m.get("role") == "statictext"]
            return "".join(ch for ch in (texts[0] if texts else "") if ch.isdigit())
    return None

results = {}
for label, binary, other in (("base", base, head), ("head", head, base)):
    window = finder_window(binary)
    finder = []
    for i in range(8):
        name = f"folder-{20 + i}"
        ref = finder_row(binary, window, name)
        ms, e = ad(binary, "click", ref)
        ok = e.get("ok") and finder_selection() == name
        if i:
            finder.append({"ms": ms, "ok": bool(ok)})
    calc = []
    ad(binary, "press", "escape", "--app", "Calculator")
    for i in range(8):
        ref = calc_button(binary, "7")
        before = calc_display(other) or ""
        ms, e = ad(binary, "click", ref)
        after = calc_display(other) or ""
        ok = e.get("ok") and after == before + "7"
        if i:
            calc.append({"ms": ms, "ok": bool(ok)})
    results[label] = {"finder_row_click": finder, "calculator_click": calc}

for case in ("finder_row_click", "calculator_click"):
    for label in ("base", "head"):
        runs = results[label][case]
        ms = [r["ms"] for r in runs]
        print(f"{case:18} {label}: min {min(ms):7.0f}  median {statistics.median(ms):7.0f}  max {max(ms):7.0f} ms  verified {sum(r['ok'] for r in runs)}/{len(runs)}")
json.dump(results, open(out, "w"), indent=1)
EOF
