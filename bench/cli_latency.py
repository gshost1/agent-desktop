#!/usr/bin/env python3
"""CLI latency and payload size per command, per benchmark target app.

No model involved. For each app and command: one warm-up run is discarded,
then N timed runs; reports min/median/max wall ms and the stdout size of the
last run. Raw outputs land in bench/out/latency/ (gitignored: they can hold
private desktop content). Summary JSON goes to bench/out/latency/summary.json
and a Markdown table to stdout.

Usage: python3 bench/cli_latency.py [--runs 9] [--bin target/release/agent-desktop]
Needs the bench lock (~/.agent-desktop/bench.lock) because it drives
Calculator and TextEdit.
"""
import argparse
import json
import os
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "out" / "latency"
BENCH_DIR = Path.home() / "agent-desktop-bench"


def run(binary, args, env, timeout=60):
    t0 = time.perf_counter()
    proc = subprocess.run([binary, *args], capture_output=True, text=True, env=env, timeout=timeout)
    ms = (time.perf_counter() - t0) * 1000
    return ms, proc.stdout


def envelope(text):
    try:
        return json.loads(text)
    except ValueError:
        return {}


def walk(node):
    yield node
    for child in node.get("children", []) or []:
        yield from walk(child)


def first_ref(env_json, pred):
    tree = env_json.get("data", {}).get("tree")
    if not tree:
        return None
    for node in walk(tree):
        if node.get("ref_id") and pred(node):
            return node["ref_id"]
    return None


def measure(label, binary, args, env, runs, results, app):
    times = []
    out = ""
    ok = None
    run(binary, args, env)
    for _ in range(runs):
        ms, out = run(binary, args, env)
        times.append(ms)
        ok = envelope(out).get("ok")
    slug = f"{app}.{label}".replace(" ", "_").replace("/", "_")
    (OUT / f"{slug}.json").write_text(out)
    row = {
        "app": app,
        "label": label,
        "args": args,
        "min_ms": round(min(times), 1),
        "median_ms": round(statistics.median(times), 1),
        "max_ms": round(max(times), 1),
        "bytes": len(out.encode()),
        "ok": ok,
        "runs": runs,
    }
    results.append(row)
    print(f"  {app:16} {label:28} min {row['min_ms']:7.1f}  med {row['median_ms']:7.1f}  "
          f"max {row['max_ms']:7.1f}  {row['bytes']:7d} B  ok={ok}", flush=True)
    return out


def snapshot_suite(binary, app, env, runs, results):
    full = measure("snapshot", binary, ["snapshot", "--app", app], env, runs, results, app)
    measure("snapshot -i", binary, ["snapshot", "--app", app, "-i"], env, runs, results, app)
    measure("snapshot -i --compact", binary, ["snapshot", "--app", app, "-i", "--compact"], env, runs, results, app)
    skel = measure("snapshot --skeleton -i --compact", binary,
                   ["snapshot", "--app", app, "--skeleton", "-i", "--compact"], env, runs, results, app)
    skel_env = envelope(skel)
    drill = first_ref(skel_env, lambda n: n.get("children_count"))
    if drill:
        measure("snapshot --root (drill) -i --compact", binary,
                ["snapshot", "--root", drill, "-i", "--compact"], env, runs, results, app)
    return envelope(full)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=9)
    ap.add_argument("--bin", default=str(ROOT.parent / "target" / "release" / "agent-desktop"))
    a = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    BENCH_DIR.mkdir(exist_ok=True)
    home = ROOT / "out" / "home-latency"
    home.mkdir(parents=True, exist_ok=True)
    os.chmod(home, 0o700)
    env = dict(os.environ, AGENT_DESKTOP_HOME=str(home))
    env.pop("AGENT_DESKTOP_SESSION", None)
    b, n, results = a.bin, a.runs, []

    print("process start")
    measure("version", b, ["version"], env, n, results, "-")
    measure("status", b, ["status"], env, n, results, "-")
    measure("list-apps", b, ["list-apps"], env, n, results, "-")
    measure("list-windows", b, ["list-windows"], env, n, results, "-")

    for app in ["Weather", "System Settings", "Finder"]:
        print(app)
        subprocess.run([b, "launch", app, "--activate"], capture_output=True, env=env)
        snapshot_suite(b, app, env, n, results)
        measure("find --role button --first", b, ["find", "--app", app, "--role", "button", "--first"], env, n, results, app)
        measure("find --text (absent)", b, ["find", "--app", app, "--text", "zzqx-not-there"], env, n, results, app)
        measure("list-surfaces", b, ["list-surfaces", "--app", app], env, n, results, app)

    print("Calculator")
    subprocess.run([b, "launch", "Calculator", "--activate"], capture_output=True, env=env)
    snap = snapshot_suite(b, "Calculator", env, n, results)
    measure("find --name 1 --exact", b, ["find", "--app", "Calculator", "--role", "button", "--name", "1", "--exact"], env, n, results, "Calculator")
    _, s = run(b, ["snapshot", "--app", "Calculator", "-i"], env)
    se = envelope(s)
    one = first_ref(se, lambda x: x.get("name") == "1" and x.get("role") == "button")
    clear = first_ref(se, lambda x: x.get("name") in ("All Clear", "Clear") and x.get("role") == "button")
    if one:
        measure("click (button)", b, ["click", one], env, n, results, "Calculator")
        measure("click --wait-for (returns snapshot)", b, ["click", one, "--wait-for", "button:All Clear"], env, n, results, "Calculator")
        measure("get value (display)", b, ["get", one, "--property", "role"], env, n, results, "Calculator")
    if clear:
        run(b, ["click", clear], env)
    measure("press escape", b, ["press", "escape"], env, n, results, "Calculator")
    measure("wait --text present", b, ["wait", "--app", "Calculator", "--text", "Percent", "--timeout", "3000"], env, n, results, "Calculator")
    measure("wait 0", b, ["wait", "0"], env, n, results, "Calculator")
    if one and clear:
        batch = json.dumps([
            {"command": "click", "args": {"ref_id": one}},
            {"command": "click", "args": {"ref_id": clear}},
            {"command": "snapshot", "args": {"app": "Calculator", "interactive_only": True, "compact": True}},
        ])
        measure("batch click+click+snapshot", b, ["batch", batch], env, n, results, "Calculator")

    print("TextEdit")
    doc = BENCH_DIR / "latency.txt"
    doc.write_text("latency bench\n")
    subprocess.run(["open", "-a", "TextEdit", str(doc)])
    time.sleep(2)
    snapshot_suite(b, "TextEdit", env, n, results)
    _, s = run(b, ["snapshot", "--app", "TextEdit", "-i"], env)
    area = first_ref(envelope(s), lambda x: x.get("role") in ("textarea", "textfield"))
    if area:
        measure("type (1 char)", b, ["type", area, "x"], env, n, results, "TextEdit")
        measure("type (40 chars)", b, ["type", area, "the quick brown fox jumps over the lazy d"], env, n, results, "TextEdit")
        measure("set-value", b, ["set-value", area, "latency bench\n"], env, n, results, "TextEdit")
    measure("press left", b, ["press", "left"], env, n, results, "TextEdit")
    subprocess.run(["osascript", "-e", 'tell application "TextEdit" to close every document saving no'], capture_output=True)

    (OUT / "summary.json").write_text(json.dumps(results, indent=1))
    print(f"\nwrote {OUT/'summary.json'}")


if __name__ == "__main__":
    main()
