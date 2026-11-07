#!/usr/bin/env bash
# How a model can choose the save folder in TextEdit's Save sheet, measured in
# both directions: each method saves one document, then the file system says
# where it landed (target folder, or a ':'-named stray in a default folder).
# Strays are moved to bench/out/icloud-strays, never deleted.
# Methods: setvalue-path (full path in the name field), type-path (headless
# type of the full path), goto (cmd+shift+g, set-value the path, return; name
# field gets only the name), goto-headed (same with --headed press).
set -u
cd "$(dirname "$0")/.."
B="${1:-bench/out/wt-base/target/release/agent-desktop}"
out=bench/out/probe_savepath; mkdir -p "$out" bench/out/icloud-strays
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
ICLOUD="$HOME/Library/Mobile Documents/com~apple~TextEdit/Documents"
ts=$(date +%H%M%S)
python3 - "$B" "$out" "$ICLOUD" "$ts" <<'PY'
import json, os, shutil, subprocess, sys, time
from pathlib import Path
B, out, icloud, ts = sys.argv[1:5]
icloud = Path(icloud); base = Path.home() / "agent-desktop-bench"
defaults = [icloud, Path.home() / "Documents", Path.home() / "Desktop"]

def ad(*args, headed=False):
    argv = [B] + (["--headed"] if headed else []) + list(args)
    t0 = time.perf_counter()
    p = subprocess.run(argv, capture_output=True, text=True, timeout=60)
    try: env = json.loads(p.stdout)
    except ValueError: env = {"raw": p.stdout[:200], "stderr": p.stderr[:200]}
    return round((time.perf_counter() - t0) * 1000), env

def walk(n):
    yield n
    for c in n.get("children", []): yield from walk(c)

def sheet():
    _, s = ad("snapshot", "--app", "TextEdit", "--surface", "sheet", "-i", "--compact", "--timeout-ms", "10000")
    return s.get("data", {}).get("tree") or {}

def node(tree, pred):
    return next((n for n in walk(tree) if pred(n)), None)

def name_field(tree):
    return node(tree, lambda n: n.get("role") == "textfield" and (n.get("name") or "").startswith("Save As") and n.get("ref_id"))

def button(tree, label):
    return node(tree, lambda n: n.get("role") == "button" and n.get("name") == label and n.get("ref_id"))

def listing(d):
    try: return {p.name for p in d.iterdir()}
    except OSError: return set()

def run(method, stem, target):
    log = {"method": method, "steps": []}
    def step(label, ms, env):
        log["steps"].append({"step": label, "ms": ms, "ok": env.get("ok"),
                             "code": (env.get("error") or {}).get("code"),
                             "delivery": ((env.get("data") or {}).get("disposition") or {}).get("delivery")})
    before = {d: listing(d) for d in defaults}
    step("launch", *ad("launch", "TextEdit", "--activate")); time.sleep(1)
    step("new", *ad("press", "cmd+n", "--app", "TextEdit")); time.sleep(1)
    step("type body", *ad("press", "h", "--app", "TextEdit"))
    step("save", *ad("press", "cmd+s", "--app", "TextEdit")); time.sleep(2)
    tree = sheet(); field = name_field(tree)
    if not field:
        log["error"] = "no Save As field"; return log
    full = str(target / stem)
    if method == "setvalue-path":
        step("set-value path", *ad("set-value", field["ref_id"], full))
    elif method == "type-path":
        step("clear", *ad("clear", field["ref_id"]))
        step("type path", *ad("type", field["ref_id"], full))
    else:
        headed = method == "goto-headed"
        step("set-value name", *ad("set-value", field["ref_id"], stem))
        step("cmd+shift+g", *ad("press", "cmd+shift+g", "--app", "TextEdit", headed=headed)); time.sleep(1.5)
        tree = sheet()
        goto = node(tree, lambda n: n.get("role") in ("textfield", "combobox") and n.get("ref_id")
                    and n is not name_field(tree) and not (n.get("name") or "").startswith("Save As")
                    and (n.get("name") or "") not in ("Tags:", "search"))
        log["goto_field"] = {k: goto.get(k) for k in ("role", "name", "value", "description")} if goto else None
        if goto:
            step("set-value goto", *ad("set-value", goto["ref_id"], str(target)))
            step("return", *ad("press", "return", "--app", "TextEdit", headed=headed)); time.sleep(1.5)
        tree = sheet()
        where = node(tree, lambda n: (n.get("name") or "").startswith("Where"))
        log["where_after_goto"] = where.get("value") if where else None
    tree = sheet(); save = button(tree, "Save")
    if save: step("click Save", *ad("click", save["ref_id"])); time.sleep(2)
    landed = sorted(p.name for p in target.iterdir()) if target.exists() else []
    strays = {str(d): sorted(n for n in listing(d) - before[d]) for d in defaults}
    strays = {d: names for d, names in strays.items() if names}
    for d, names in strays.items():
        for n in names:
            if stem in n:
                shutil.move(str(Path(d) / n), "bench/out/icloud-strays/" + n)
    log.update({"target_contents": landed, "strays": strays,
                "landed_in_target": any(Path(n).stem == stem for n in landed)})
    ad("press", "escape", "--app", "TextEdit"); time.sleep(0.5)
    subprocess.run(["osascript", "-e", 'tell application "TextEdit" to close front document saving no'], capture_output=True, timeout=20)
    return log

results = []
for method in ("setvalue-path", "type-path", "goto", "goto-headed"):
    target = base / f"savepath-{ts}-{method}"
    target.mkdir(parents=True, exist_ok=True)
    r = run(method, f"sp-{ts}-{method}", target); results.append(r)
    print(json.dumps({k: r.get(k) for k in ("method", "landed_in_target", "target_contents", "strays", "goto_field", "where_after_goto", "error")}))
    print("   ", [(s["step"], s["ms"], s["ok"], s["code"]) for s in r["steps"]])
json.dump(results, open(f"{out}/results-{ts}.json", "w"), indent=1)
PY
