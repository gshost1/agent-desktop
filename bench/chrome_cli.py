#!/usr/bin/env python3
"""CLI latency and payload on Chromium content, no model.

Opens the ChromeRead invoice page in a throwaway-profile Chrome, measures the
cold first snapshot (accessibility activation path) once, then warm commands
over N runs with the warm-up discarded. Raw outputs: bench/out/chrome/.

Usage: python3 bench/chrome_cli.py [--runs 9]
"""
import argparse
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
from tasks import ChromeRead  # noqa: E402

BIN = str(ROOT.parent / "target" / "release" / "agent-desktop")
OUT = ROOT / "out" / "chrome"
APP = "Google Chrome"


def run(args, env):
    t0 = time.perf_counter()
    p = subprocess.run([BIN, *args], capture_output=True, text=True, env=env, timeout=90)
    return (time.perf_counter() - t0) * 1000, p.stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=9)
    a = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    home = ROOT / "out" / "home-chrome"
    home.mkdir(parents=True, exist_ok=True)
    os.chmod(home, 0o700)
    env = dict(os.environ, AGENT_DESKTOP_HOME=str(home))
    task = ChromeRead("chromecli" + time.strftime("%H%M%S"), BIN)
    task.setup()
    rows = []
    try:
        ms, out = run(["snapshot", "--app", APP, "-i", "--compact"], env)
        (OUT / "cold.json").write_text(out)
        rows.append({"label": "cold first snapshot -i --compact", "min_ms": round(ms, 1), "median_ms": round(ms, 1),
                     "max_ms": round(ms, 1), "bytes": len(out.encode()), "ok": json.loads(out or "{}").get("ok")})
        print(rows[-1], flush=True)
        cases = [
            ("snapshot", ["snapshot", "--app", APP]),
            ("snapshot -i", ["snapshot", "--app", APP, "-i"]),
            ("snapshot -i --compact", ["snapshot", "--app", APP, "-i", "--compact"]),
            ("snapshot --skeleton -i --compact", ["snapshot", "--app", APP, "--skeleton", "-i", "--compact"]),
            ("find --text 'Order total'", ["find", "--app", APP, "--text", "Order total"]),
            ("find --role button --name 'Pay now'", ["find", "--app", APP, "--role", "button", "--name", "Pay now"]),
        ]
        for label, args in cases:
            run(args, env)
            times, out = [], ""
            for _ in range(a.runs):
                ms, out = run(args, env)
                times.append(ms)
            (OUT / (label.replace(" ", "_").replace("'", "") + ".json")).write_text(out)
            rows.append({"label": label, "min_ms": round(min(times), 1), "median_ms": round(statistics.median(times), 1),
                         "max_ms": round(max(times), 1), "bytes": len(out.encode()), "ok": json.loads(out or "{}").get("ok")})
            print(rows[-1], flush=True)
    finally:
        task.cleanup()
    (OUT / "summary.json").write_text(json.dumps(rows, indent=1))


if __name__ == "__main__":
    main()
