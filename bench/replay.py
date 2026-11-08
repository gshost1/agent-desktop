#!/usr/bin/env python3
"""Model-free replay of one recorded bench run: its exact Bash commands, in order.

Sets the task up as the driver does, then runs every Bash command the agent ran,
with the run's name token swapped for a fresh one and each recorded snapshot id
swapped for the one the replay created at the same position (snapshots are
matched in creation order through each run's AGENT_DESKTOP_HOME store). Every
agent-desktop call is timed by the same bin/agent-desktop wrapper the driver
uses. Prints the slowest calls with their output size, then the task's own check.

A replay measures the CLI's cost on a fixed command sequence. When a fix changes
what a command returns, later commands can diverge; the report counts failing
calls so a divergence is visible.

Usage: python3 bench/replay.py --run <run_id> [--tree bench/out/wt-fork] [--top 10]
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
from tasks import TASKS, reset_desktop  # noqa: E402

LOCK = Path.home() / ".agent-desktop" / "bench.lock"
SNAPSHOT_REF = re.compile(r"@(s[0-9a-z]+):")
COMMAND_TIMEOUT_S = 90


def recorded_commands(run_dir):
    commands = []
    for line in (run_dir / "stream.jsonl").read_text().splitlines():
        ev = json.loads(line)
        content = (ev.get("message") or {}).get("content")
        if ev.get("type") != "assistant" or not isinstance(content, list):
            continue
        for block in content:
            if block.get("type") == "tool_use" and block.get("name") == "Bash":
                commands.append(block["input"].get("command", ""))
    return commands


def snapshots_in_order(home):
    store = home / "snapshots"
    if not store.exists():
        return []
    return [p.name for p in sorted(store.iterdir(), key=lambda p: p.stat().st_mtime)]


def remap(command, mapping):
    return SNAPSHOT_REF.sub(lambda m: "@" + mapping.get(m.group(1), m.group(1)) + ":", command)


def read_calls(log):
    calls = []
    if not log.exists():
        return calls
    for line in log.read_text(errors="replace").splitlines():
        parts = line.split("\t")
        if len(parts) >= 5:
            calls.append({"s": float(parts[1]) - float(parts[0]), "rc": int(parts[2]),
                          "bytes": int(parts[3]), "args": parts[4]})
    return calls


def replay(run_id, tree, top):
    source = ROOT / "runs" / run_id
    summary = json.loads((source / "summary.json").read_text())
    task_key = summary["task"]
    real_bin = (Path(tree).resolve() if tree else ROOT.parent) / "target" / "release" / "agent-desktop"
    if not real_bin.exists():
        sys.exit(f"no release binary at {real_bin}")
    old_token = run_id.replace("-", "")
    replay_id = time.strftime("%m%d%H%M%S") + "-" + uuid.uuid4().hex[:4]
    new_token = replay_id.replace("-", "")
    out = ROOT / "out" / "replay" / replay_id
    home = out / "ad-home"
    home.mkdir(parents=True, mode=0o700)
    cwd = out / "cwd"
    cwd.mkdir()
    task = TASKS[task_key](new_token, str(real_bin))
    reset_desktop()
    time.sleep(1.5)
    task.setup()
    env = dict(os.environ, PATH=f"{ROOT / 'bin'}:{os.environ['PATH']}", AD_REAL=str(real_bin),
               AD_LOG=str(out / "ad.log"), AGENT_DESKTOP_HOME=str(home))
    env.pop("AGENT_DESKTOP_SESSION", None)
    recorded = snapshots_in_order(source / "ad-home")
    commands = recorded_commands(source)
    t0 = time.time()
    for command in commands:
        live = snapshots_in_order(home)
        mapping = dict(zip(recorded, live))
        text = remap(command.replace(old_token, new_token), mapping)
        try:
            subprocess.run(["bash", "-c", text], cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                           capture_output=True, timeout=COMMAND_TIMEOUT_S)
        except subprocess.TimeoutExpired:
            pass
    wall = time.time() - t0
    try:
        ok, why = task.verify("DONE")
    except Exception as exc:
        ok, why = False, f"verify error {exc}"
    try:
        task.cleanup()
    except Exception as exc:
        print(f"cleanup failed: {exc}")
    calls = read_calls(out / "ad.log")
    report = {"replay_id": replay_id, "source_run": run_id, "task": task_key, "binary": str(real_bin),
              "commands": len(commands), "cli_calls": len(calls), "failed_calls": sum(c["rc"] != 0 for c in calls),
              "wall_s": round(wall, 1), "cli_s": round(sum(c["s"] for c in calls), 1),
              "cli_bytes": sum(c["bytes"] for c in calls), "task_check": ok, "verify": why,
              "slowest": sorted(calls, key=lambda c: -c["s"])[:top]}
    (out / "report.json").write_text(json.dumps(report, indent=1))
    print(f"replay {replay_id} of {run_id} ({task_key}) on {real_bin}")
    print(f"commands {len(commands)}, agent-desktop calls {len(calls)} ({report['failed_calls']} failed), "
          f"wall {report['wall_s']} s, CLI {report['cli_s']} s, output {report['cli_bytes'] // 1024} KB, "
          f"task check {'pass' if ok else 'fail'} ({why})")
    for c in report["slowest"]:
        print(f"{c['s']:6.2f} s  rc={c['rc']}  {c['bytes']:7d} B  {c['args'][:150]}")
    return report


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--tree", default=None, help="checkout whose release binary to use")
    ap.add_argument("--top", type=int, default=10)
    a = ap.parse_args()
    if not (LOCK.exists() and LOCK.read_text().startswith("perf/speed-cost")):
        sys.exit(f"take the bench lock first: {LOCK}")
    replay(a.run, a.tree, a.top)


if __name__ == "__main__":
    main()
