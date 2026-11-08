#!/usr/bin/env bash
# How a model can choose the save folder in TextEdit's Save sheet, measured in
# both directions: each method saves one document, then the file system says
# where it landed (target folder, or a ':'-named stray in a default folder).
# Strays are moved to bench/out/icloud-strays, never deleted.
# Methods: setvalue-path (full path in the name field), type-path (headless
# type of the full path), goto (cmd+shift+g, set-value the path, return; name
# field gets only the name), goto-headed (same with --headed press).
set -u
echo "probe_savepath.sh is disabled: it lists ~/Documents, ~/Desktop and iCloud Drive and browses from the home folder, which bench/README.md forbids." >&2
exit 1
cd "$(dirname "$0")/.."
B="${1:-bench/out/wt-popup/target/release/agent-desktop}"
METHODS="${2:-goto browse}"
out=bench/out/probe_savepath; mkdir -p "$out" bench/out/icloud-strays
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
ICLOUD="$HOME/Library/Mobile Documents/com~apple~TextEdit/Documents"
ts=$(date +%H%M%S)
python3 - "$B" "$out" "$ICLOUD" "$ts" "$METHODS" <<'PY'
import json, os, shutil, subprocess, sys, time
from pathlib import Path
B, out, icloud, ts, methods = sys.argv[1:6]
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
    elif method.startswith("goto"):
        headed = method == "goto-headed"
        step("set-value name", *ad("set-value", field["ref_id"], stem))
        step("cmd+shift+g", *ad("press", "cmd+shift+g", "--app", "TextEdit", headed=headed)); time.sleep(1.5)
        _, surfaces = ad("list-surfaces", "--app", "TextEdit")
        log["surfaces_after_goto"] = [(x.get("kind"), x.get("title") or x.get("name")) for x in (surfaces.get("data") or {}).get("surfaces", [])] if isinstance(surfaces.get("data"), dict) else surfaces.get("data")
        _, whole = ad("snapshot", "--app", "TextEdit", "-i", "--compact", "--timeout-ms", "10000")
        json.dump(whole, open(f"{out}/goto-{ts}-{method}-app.json", "w"))
        known = {n.get("ref_id") for n in walk(sheet())}
        fresh = [n for n in walk((whole.get("data") or {}).get("tree") or {}) if n.get("role") in ("textfield", "combobox") and n.get("ref_id")]
        log["app_text_fields_after_goto"] = [{k: n.get(k) for k in ("role", "name", "value", "description")} for n in fresh][:12]
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
    if method == "browse":
        step("set-value name", *ad("set-value", field["ref_id"], stem))
        tree = sheet()
        toggles = [n for n in walk(tree) if n.get("ref_id") and n.get("role") in ("disclosure", "disclosuretriangle", "button")
                   and any(w in ((n.get("name") or "") + " " + (n.get("description") or "")).lower() for w in ("expand", "more options", "disclosure", "show"))]
        log["expand_candidates"] = [{k: n.get(k) for k in ("role", "name", "description", "native_id")} for n in toggles][:6]
        if not node(tree, lambda n: n.get("role") == "browser") and toggles:
            step("expand", *ad("click", toggles[0]["ref_id"])); time.sleep(1.5)
        tree = sheet()
        cells = lambda t: [n for n in walk(t) if n.get("role") == "textfield" and n.get("ref_id")]
        start = next((n for n in cells(tree) if n.get("value") == "~"), None)
        log["browser_present"] = bool(node(tree, lambda n: n.get("role") == "browser"))
        log["browse_start"] = "~" if start else None
        if not start:
            where = node(tree, lambda n: (n.get("name") or "").startswith("Where") and n.get("ref_id"))
            if where:
                step("select home in Where", *ad("select", where["ref_id"], Path.home().name)); time.sleep(1.5)
                tree = sheet(); start = next((n for n in cells(tree) if n.get("value") == "~"), None)
                log["browse_start"] = "~ after Where" if start else None
        if start:
            step("click ~", *ad("click", start["ref_id"])); time.sleep(1)
            for part in target.relative_to(Path.home()).parts:
                tree = sheet(); cell = next((n for n in cells(tree) if n.get("value") == part), None)
                if not cell:
                    log["error"] = f"no column cell {part}"; break
                step(f"click {part}", *ad("click", cell["ref_id"])); time.sleep(1)
        tree = sheet()
        where = node(tree, lambda n: (n.get("name") or "").startswith("Where"))
        log["where_after_browse"] = where.get("value") if where else None
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
    subprocess.run(["bench/out/macctl", "close-front-discard", "TextEdit"], capture_output=True, timeout=20)
    return log

results = []
for method in methods.split():
    target = base / f"savepath-{ts}-{method}"
    target.mkdir(parents=True, exist_ok=True)
    r = run(method, f"sp-{ts}-{method}", target); results.append(r)
    print(json.dumps({k: r.get(k) for k in ("method", "landed_in_target", "target_contents", "strays", "goto_field", "where_after_goto", "surfaces_after_goto", "app_text_fields_after_goto", "expand_candidates", "browser_present", "browse_start", "where_after_browse", "error")}))
    print("   ", [(s["step"], s["ms"], s["ok"], s["code"]) for s in r["steps"]])
json.dump(results, open(f"{out}/results-{ts}.json", "w"), indent=1)
PY
