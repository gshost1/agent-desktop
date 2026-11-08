#!/usr/bin/env bash
# Live check of a headless click on a Finder icon-view folder item: base and
# head binaries each click one folder; the effect is read independently from
# Finder (the front window's target) and the envelope's ok/delivery is recorded.
# Usage: bash bench/check_open_click.sh <base-binary> <head-binary> [out.json]
set -u
cd "$(dirname "$0")/.."
base="$1"; head="$2"; out="${3:-bench/out/check_open_click.json}"
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-ab"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
D="$HOME/agent-desktop-bench/probe-many"
mkdir -p "$D"; for i in $(seq 1 60); do mkdir -p "$D/folder-$i"; done
python3 - "$base" "$head" "$out" "$D" <<'PY'
import json, subprocess, sys, time
base, head, out, D = sys.argv[1:5]

def macctl(*args):
    return subprocess.run(["bench/out/macctl", *args], capture_output=True, text=True, timeout=30).stdout.strip()

def ad(binary, *args):
    t0 = time.perf_counter()
    p = subprocess.run([binary, *args], capture_output=True, text=True, timeout=120)
    try:
        return (time.perf_counter() - t0) * 1000, json.loads(p.stdout)
    except ValueError:
        return (time.perf_counter() - t0) * 1000, {}

def walk(n):
    yield n
    for c in n.get("children", []):
        yield from walk(c)

def reset_window():
    macctl("close-discard", "Finder", "probe-many", "folder-")
    subprocess.run(["open", D]); time.sleep(3)
    macctl("raise", "Finder", "probe-many"); macctl("menu", "Finder", "View", "as Icons")
    time.sleep(1)

def window_id(binary):
    _, w = ad(binary, "list-windows", "--app", "Finder")
    for win in w.get("data", []):
        if win.get("title") == "probe-many":
            return win["id"]
    for win in w.get("data", []):
        _, s = ad(binary, "snapshot", "--window-id", win["id"], "--max-depth", "1")
        if s.get("data", {}).get("tree", {}).get("name") == "probe-many":
            return win["id"]
    return None

results = {}
for label, binary, name in (("base", base, "folder-31"), ("head", head, "folder-32")):
    reset_window()
    wid = window_id(binary)
    if not wid:
        results[label] = {"error": "probe-many window not found"}; continue
    _, s = ad(binary, "snapshot", "--window-id", wid, "-i", "--timeout-ms", "20000")
    items = [n for n in walk(s.get("data", {}).get("tree", {}))
             if n.get("ref_id") and n.get("role") in ("image", "textfield", "statictext")
             and (n.get("name") or n.get("value")) == name]
    target = next((n for role in ("textfield", "statictext", "image") for n in items if n.get("role") == role), None)
    if not target:
        results[label] = {"error": f"no icon-view item for {name}"}; continue
    ms, e = ad(binary, "click", target["ref_id"])
    time.sleep(1)
    front = (json.loads(macctl("windows", "Finder") or "[]") or [""])[0]
    results[label] = {"role": target["role"], "ms": round(ms), "ok": e.get("ok"),
                      "error": (e.get("error") or {}).get("code"),
                      "disposition": (e.get("error") or {}).get("disposition") or (e.get("data") or {}).get("delivery"),
                      "data": e.get("data"), "front_window": front, "opened": front == name}
    print(label, json.dumps(results[label])[:400])
json.dump(results, open(out, "w"), indent=1)
PY
