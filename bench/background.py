#!/usr/bin/env python3
"""Background and multi-app check, no model.

A sentinel app (Dictionary) is frontmost with its search field focused. Three
workers drive Calculator, TextEdit and Weather in parallel, each under its own
--session and --agent-id, with headless commands only. A sampler records the
frontmost app, the focused element of the frontmost app and the cursor position
every 100 ms, independently of agent-desktop (bench/focus_probe.swift, AX and
AppKit directly, no Automation permission). Effects are checked
by independent reads: Calculator's display, the TextEdit file on disk after
save is not used; the document text is read back from TextEdit by AppleScript.

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
MACCTL = str(ROOT / "out" / "macctl")
BIN = str(ROOT.parent / "target" / "release" / "agent-desktop")
SENTINEL = "Dictionary"
FOCUS_PROBE = str(ROOT / "out" / "focus_probe")


def build_focus_probe():
    src = ROOT / "focus_probe.swift"
    if not Path(FOCUS_PROBE).exists() or Path(FOCUS_PROBE).stat().st_mtime < src.stat().st_mtime:
        subprocess.run(["swiftc", "-O", str(src), "-o", FOCUS_PROBE], check=True)


def probe():
    try:
        out = subprocess.run([FOCUS_PROBE, "--once"], capture_output=True, text=True, timeout=10).stdout
        return json.loads(out)
    except (ValueError, subprocess.TimeoutExpired):
        return {"front": "?", "x": None, "y": None, "focus": "?"}


class Sampler(threading.Thread):
    def __init__(self):
        super().__init__(daemon=True)
        self.samples, self.stop = [], threading.Event()

    def run(self):
        proc = subprocess.Popen([FOCUS_PROBE], stdout=subprocess.PIPE, text=True)
        for line in proc.stdout:
            if self.stop.is_set():
                break
            try:
                self.samples.append(json.loads(line))
            except ValueError:
                pass
        proc.terminate()


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
    return subprocess.run([MACCTL, "text", "TextEdit"], capture_output=True, text=True).stdout.strip()


def make_frontmost():
    subprocess.run(["open", "-a", SENTINEL])
    time.sleep(1.5)
    subprocess.run([MACCTL, "activate", SENTINEL])
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
    subprocess.run([MACCTL, "quit", "Calculator"])
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
    build_focus_probe()
    from tasks import build_macctl
    build_macctl()
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
    subprocess.run([MACCTL, "close-discard", "TextEdit", ""])
    (OUT / "summary.json").write_text(json.dumps(results, indent=1, default=str))


if __name__ == "__main__":
    main()
