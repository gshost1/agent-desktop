#!/usr/bin/env python3
"""Held-out native run: one task, one arm, one `claude -p` run per --runs, on the subscription.

Same loop as bench/driver.py (Bash only, bypassPermissions, stream-json, per-call tool log,
independent verify, forbidden-program audit), generalised to any tool arm. Rows go to
heldout/runs/results-native.jsonl.

The caller holds the desktop lock (bash bench/lockq.sh take "perf/speed-cost heldout" 5).

Usage: python3.11 -I heldout/arms/run_native.py --arm ad-14 --task hn1 [--runs 1] [--round 0]
       [--model claude-opus-5-5] [--screen-recording]
"""
import argparse
import json
import os
import signal
import subprocess
import sys
import threading
import time
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import common  # noqa: E402

sys.path.insert(0, str(common.BENCH))
import driver  # noqa: E402
import tasks  # noqa: E402
import tasks_heldout as th  # noqa: E402
import desktop  # noqa: E402

desktop.install(th)

RUNS = common.HELDOUT / "runs"


def reset_heldout_desktop():
    """Quit System Settings (only ever read) and the bench documents' apps. An app that shows any
    window that is not a bench window stops the run before the model starts; it is never touched."""
    tasks.quit_app("System Settings")
    for app in ("TextEdit", "Preview"):
        th.close_bench_app(app)


def post_arm_cleanup(arm, rdir):
    """Stop daemons the arm started for this run; they must not carry state into the next run."""
    if arm == "peekaboo":
        env = dict(os.environ, HO_ARM_HOME=str(rdir / "arm-home"))
        subprocess.run([str(HERE / "bin/peekaboo/peekaboo"), "daemon", "stop"], env=env, capture_output=True, timeout=30)
    if arm == "cua":
        subprocess.run([str(HERE / "bin/cua/cua-driver"), "call", "end_session", "{}"], capture_output=True, timeout=30)


def run_once(a):
    run_id = time.strftime("%m%d%H%M%S") + "-" + uuid.uuid4().hex[:4]
    rdir = RUNS / "native" / run_id
    rdir.mkdir(parents=True)
    task = th.NATIVE_TASKS[a.task](run_id.replace("-", ""), str(common.VERIFY_AD))
    reset_heldout_desktop()
    time.sleep(1.5)
    task.setup()
    cwd = rdir / "cwd"
    cwd.mkdir()
    home = rdir / "ad-home"
    home.mkdir(mode=0o700)
    (rdir / "arm-home").mkdir()
    env = dict(os.environ)
    env.update({"PATH": f"{common.arm_bin_dir(a.arm)}:{env['PATH']}", "AD_LOG": str(rdir / "ad.log"),
                "AGENT_DESKTOP_HOME": str(home), "HO_ARM_HOME": str(rdir / "arm-home")})
    env.pop("AGENT_DESKTOP_SESSION", None)
    cmd = ["claude", "-p", common.TASK_PREFIX + task.prompt(), "--output-format", "stream-json", "--verbose",
           "--setting-sources", "project", "--strict-mcp-config", "--tools", "Bash",
           "--permission-mode", "bypassPermissions", "--model", a.model, "--no-session-persistence",
           "--append-system-prompt", common.native_system_prompt(a.arm, a.screen_recording)]
    t0 = time.time()
    events = []
    proc = subprocess.Popen(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            stdin=subprocess.DEVNULL, text=True)
    capped = threading.Event()

    def stop_at_cap():
        capped.set()
        proc.kill()

    watchdog = threading.Timer(th.NATIVE_CAP_S, stop_at_cap)
    watchdog.daemon = True
    watchdog.start()
    with open(rdir / "stream.jsonl", "w") as fh:
        for line in proc.stdout:
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            ev["_t"] = round(time.time() - t0, 3)
            events.append(ev)
            fh.write(json.dumps(ev) + "\n")
    proc.wait()
    watchdog.cancel()
    wall = time.time() - t0
    stderr = proc.stderr.read()
    summary = driver.summarize(events, rdir, wall)
    answer = summary.get("answer", "")
    try:
        ok, why = task.verify(answer)
    except Exception as exc:
        ok, why = False, f"verify error {exc}"
    forbidden = [f for f in (common.audit_native(c, a.arm, th.FORBIDDEN) for c in summary["commands"]) if f]
    if forbidden:
        ok, why = False, f"forbidden command {forbidden[0]}; {why}"
    if capped.is_set():
        ok, why = False, f"timed out at the {th.NATIVE_CAP_S} s cap; {why}"
    del summary["commands"]
    summary.update({"suite": "native", "arm": a.arm, "arm_version": common.ARM_VERSIONS[a.arm], "run_id": run_id,
                    "task": a.task, "round": a.round, "model": a.model, "success": ok, "verify": why,
                    "cap_s": th.NATIVE_CAP_S, "timed_out": capped.is_set(), "screen_recording": a.screen_recording,
                    "host": a.host})
    summary.update(driver.validity(stderr, rdir))
    (rdir / "summary.json").write_text(json.dumps(summary, indent=1))
    with open(RUNS / "results-native.jsonl", "a") as fh:
        fh.write(json.dumps(summary) + "\n")
    print(f"{run_id} {a.arm:8} {a.task} ok={ok!s:5} wall={wall:6.1f}s cli={summary['cli_s']:5.1f}s "
          f"turns={summary.get('num_turns')} in={summary['input_total']} | {why}", flush=True)
    for step in (task.cleanup, lambda: post_arm_cleanup(a.arm, rdir)):
        try:
            step()
        except Exception as exc:
            print(f"{run_id} cleanup failed after the row was written: {exc}", flush=True)
    return summary


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm", required=True, choices=sorted(common.NATIVE_TOOL))
    ap.add_argument("--task", required=True, choices=sorted(th.NATIVE_TASKS))
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--round", type=int, default=0)
    ap.add_argument("--model", default="claude-opus-5-5")
    ap.add_argument("--screen-recording", action="store_true", help="only on a host where every arm has it granted")
    ap.add_argument("--host", default="mac")
    a = ap.parse_args()
    if not driver.holds_lock():
        sys.exit(f"take the bench lock first: {driver.LOCK}")
    RUNS.mkdir(exist_ok=True)
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    for _ in range(a.runs):
        run_once(a)


if __name__ == "__main__":
    main()
