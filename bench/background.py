#!/usr/bin/env python3
"""Background and multi-app check, no model.

A sentinel app (Dictionary) is frontmost with its search field focused. Three
workers drive Calculator, TextEdit and Weather in parallel, each under its own
--session and --agent-id, with headless commands only. A sampler records the
frontmost app, the focused element of the frontmost app and the cursor position
every 100 ms, independently of agent-desktop (JXA/AppKit). Effects are checked
by independent reads: Calculator's display, the TextEdit file on disk after
save is not used; the document text is read back through System Events.

Phases: serial (one worker at a time) then parallel (all at once), so the
latency cost of the shared interaction lease shows. Extra probes: `launch`
without --activate, and headless `press` while another app is frontmost.

Usage: python3 bench/background.py [--iterations 5]
Writes bench/out/background/summary.json. Needs the bench lock.
"""
import argparse
import json
import os
import statistics
import subprocess
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "out" / "background"
BIN = str(ROOT.parent / "target" / "release" / "agent-desktop")
SENTINEL = "Dictionary"
PROBE = """
ObjC.import("AppKit");
var p = $.NSEvent.mouseLocation;
var front = $.NSWorkspace.sharedWorkspace.frontmostApplication.localizedName.js;
var se = Application("System Events");
var focus = "";
try {
  var proc = se.processes.whose({frontmost: true})[0];
  var el = proc.attributes.byName("AXFocusedUIElement").value();
  focus = el.role() + ":" + (el.description() || "");
} catch (e) { focus = "?"; }
JSON.stringify({front: front, x: Math.round(p.x), y: Math.round(p.y), focus: focus});
"""


def probe():
    out = subprocess.run(["osascript", "-l", "JavaScript", "-e", PROBE], capture_output=True, text=True, timeout=10).stdout
    try:
        return json.loads(out)
    except ValueError:
        return {"front": "?", "x": None, "y": None, "focus": "?"}


class Sampler(threading.Thread):
    def __init__(self):
        super().__init__(daemon=True)
        self.samples, self.stop = [], threading.Event()

    def run(self):
        while not self.stop.is_set():
            s = probe()
            s["t"] = time.time()
            self.samples.append(s)
            time.sleep(0.1)


def ad(args, env):
    t0 = time.perf_counter()
    p = subprocess.run([BIN, *args], capture_output=True, text=True, env=env, timeout=60)
    ms = (time.perf_counter() - t0) * 1000
    try:
        e = json.loads(p.stdout)
    except ValueError:
        e = {}
    return ms, e


def walk(n):
    yield n
    for c in n.get("children", []) or []:
        yield from walk(c)


def refs(env_json, pred):
    tree = env_json.get("data", {}).get("tree") or {}
    return [n["ref_id"] for n in walk(tree) if n.get("ref_id") and pred(n)]


def session_env(name, home):
    env = dict(os.environ, AGENT_DESKTOP_HOME=str(home))
    _, e = ad(["session", "start", "--name", name, "--no-trace"], env)
    sid = e.get("data", {}).get("session_id") or e.get("data", {}).get("id")
    env["AGENT_DESKTOP_SESSION"] = sid or ""
    env["AGENT_DESKTOP_AGENT_ID"] = name
    return env


def calculator_worker(env, iters, log):
    for _ in range(iters):
        ms, s = ad(["snapshot", "--app", "Calculator", "-i", "--compact"], env)
        log.append(("calc snapshot", ms, s.get("ok")))
        tree = s.get("data", {}).get("tree") or {}
        by = {n.get("name"): n.get("ref_id") for n in walk(tree) if n.get("ref_id")}
        for key in ["All Clear", "Clear"]:
            if by.get(key):
                ms, r = ad(["click", by[key]], env)
                log.append(("calc click", ms, r.get("ok")))
                break
        for key in ["1", "2", "3", "Add", "4", "Equals"]:
            if by.get(key):
                ms, r = ad(["click", by[key]], env)
                log.append(("calc click", ms, r.get("ok")))


def textedit_worker(env, iters, log, area_ref_holder):
    for i in range(iters):
        ms, s = ad(["snapshot", "--app", "TextEdit", "-i", "--compact"], env)
        log.append(("textedit snapshot", ms, s.get("ok")))
        areas = refs(s, lambda n: n.get("role") == "textarea")
        if not areas:
            continue
        ms, r = ad(["set-value", areas[0], f"background run {i}"], env)
        log.append(("textedit set-value", ms, r.get("ok")))
        ms, r = ad(["type", areas[0], " typed"], env)
        log.append(("textedit type", ms, r.get("ok")))
        area_ref_holder[0] = areas[0]


def weather_worker(env, iters, log):
    for _ in range(iters):
        ms, s = ad(["snapshot", "--app", "Weather", "-i", "--compact"], env)
        log.append(("weather snapshot", ms, s.get("ok")))
        ms, f = ad(["find", "--app", "Weather", "--role", "button", "--text", "°", "--first"], env)
        log.append(("weather find", ms, f.get("ok")))


def calculator_display(env):
    _, s = ad(["snapshot", "--app", "Calculator"], env)
    tree = s.get("data", {}).get("tree") or {}
    for n in walk(tree):
        if n.get("role") == "scrollarea" and n.get("name") == "Edit field":
            texts = [m.get("value") or m.get("name") for m in walk(n) if m.get("role") == "statictext"]
            return "".join(ch for ch in (texts[0] if texts else "") if ch.isdigit() or ch == ".")
    return None


def textedit_text():
    script = 'tell application "TextEdit" to get text of document 1'
    return subprocess.run(["osascript", "-e", script], capture_output=True, text=True).stdout.strip()


def make_frontmost():
    subprocess.run(["open", "-a", SENTINEL])
    time.sleep(1.5)
    subprocess.run(["osascript", "-e", f'tell application "{SENTINEL}" to activate'])
    time.sleep(1.0)


def stats(log):
    by = {}
    for name, ms, ok in log:
        by.setdefault(name, []).append((ms, ok))
    return {k: {"n": len(v), "ok": sum(1 for _, o in v if o), "min_ms": round(min(m for m, _ in v), 1),
                "median_ms": round(statistics.median(m for m, _ in v), 1), "max_ms": round(max(m for m, _ in v), 1)}
            for k, v in by.items()}


def run_phase(name, parallel, iters, envs):
    make_frontmost()
    before = probe()
    sampler = Sampler()
    sampler.start()
    logs = {"calc": [], "textedit": [], "weather": []}
    holder = [None]
    jobs = [
        threading.Thread(target=calculator_worker, args=(envs["calc"], iters, logs["calc"])),
        threading.Thread(target=textedit_worker, args=(envs["textedit"], iters, logs["textedit"], holder)),
        threading.Thread(target=weather_worker, args=(envs["weather"], iters, logs["weather"])),
    ]
    t0 = time.perf_counter()
    if parallel:
        for j in jobs:
            j.start()
        for j in jobs:
            j.join()
    else:
        for j in jobs:
            j.start()
            j.join()
    wall = time.perf_counter() - t0
    sampler.stop.set()
    sampler.join()
    after = probe()
    fronts = sorted({s["front"] for s in sampler.samples})
    cursors = sorted({(s["x"], s["y"]) for s in sampler.samples})
    focuses = sorted({s["focus"] for s in sampler.samples})
    display = calculator_display(envs["calc"])
    text = textedit_text()
    result = {
        "phase": name, "wall_s": round(wall, 2), "before": before, "after": after,
        "frontmost_seen": fronts, "cursor_positions_seen": len(cursors), "cursor_first_last": [cursors[0], cursors[-1]] if cursors else None,
        "focus_seen": focuses, "samples": len(sampler.samples),
        "calculator_display": display, "calculator_ok": display == "127",
        "textedit_text": text[:80], "textedit_ok": text.endswith(" typed") and text.startswith("background run"),
        "latency": {k: stats(v) for k, v in logs.items()},
    }
    print(json.dumps({k: result[k] for k in ["phase", "wall_s", "before", "after", "frontmost_seen", "cursor_positions_seen",
                                            "focus_seen", "calculator_display", "textedit_text"]}), flush=True)
    for k, v in result["latency"].items():
        for cmd, st in v.items():
            print(f"   {cmd:22} n={st['n']:3} ok={st['ok']:3} min {st['min_ms']:7.1f} med {st['median_ms']:7.1f} max {st['max_ms']:7.1f}", flush=True)
    return result


def launch_probe(env):
    subprocess.run(["osascript", "-e", 'if application "Calculator" is running then tell application "Calculator" to quit'])
    time.sleep(1.5)
    make_frontmost()
    before = probe()
    ms, r = ad(["launch", "Calculator"], env)
    time.sleep(1.0)
    after = probe()
    res = {"probe": "launch without --activate", "ms": round(ms, 1), "ok": r.get("ok"), "before": before, "after": after}
    print(json.dumps(res), flush=True)
    return res


def press_probe(env):
    make_frontmost()
    before = probe()
    ms, r = ad(["press", "escape", "--app", "Calculator"], env)
    after = probe()
    ms2, r2 = ad(["press", "escape"], env)
    after2 = probe()
    res = {"probe": "headless press while another app is frontmost",
           "scoped": {"ms": round(ms, 1), "ok": r.get("ok"), "error": (r.get("error") or {}).get("code"), "after": after},
           "unscoped": {"ms": round(ms2, 1), "ok": r2.get("ok"), "error": (r2.get("error") or {}).get("code"), "after": after2},
           "before": before}
    print(json.dumps(res), flush=True)
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--iterations", type=int, default=5)
    a = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    home = ROOT / "out" / "home-background"
    home.mkdir(parents=True, exist_ok=True)
    os.chmod(home, 0o700)
    doc = Path.home() / "agent-desktop-bench" / "background.txt"
    doc.parent.mkdir(exist_ok=True)
    doc.write_text("background\n")
    for app in ["Calculator", "Weather"]:
        subprocess.run(["open", "-g", "-a", app])
    subprocess.run(["open", "-g", "-a", "TextEdit", str(doc)])
    time.sleep(4)
    envs = {k: session_env(f"bg-{k}", home) for k in ["calc", "textedit", "weather"]}
    results = {"launch": launch_probe(envs["calc"])}
    time.sleep(2)
    results["serial"] = run_phase("serial", False, a.iterations, envs)
    results["parallel"] = run_phase("parallel", True, a.iterations, envs)
    results["press"] = press_probe(envs["calc"])
    subprocess.run(["osascript", "-e", 'tell application "TextEdit" to close every document saving no'])
    (OUT / "summary.json").write_text(json.dumps(results, indent=1, default=str))


if __name__ == "__main__":
    main()
