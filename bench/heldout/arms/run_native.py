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
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import common  # noqa: E402
import cua_daemon  # noqa: E402
import run_audit  # noqa: E402
import run_guard  # noqa: E402

sys.path.insert(0, str(common.BENCH))
import driver  # noqa: E402
import tasks  # noqa: E402
import desktop  # noqa: E402

th = common.load_taskset()

VERIFY_HOME = tempfile.mkdtemp(prefix="ho-dialogs-")


def verify_ad(*args):
    env = dict(os.environ, AGENT_DESKTOP_HOME=VERIFY_HOME)
    env.pop("AGENT_DESKTOP_SESSION", None)
    out = subprocess.run([str(common.VERIFY_AD), *args], capture_output=True, text=True, env=env, timeout=60)
    try:
        return json.loads(out.stdout)
    except ValueError:
        return {}


def dialog_lines(app):
    return (verify_ad("snapshot", "--app", app, "--lines").get("data") or {}).get("lines") or []


DESKTOP = desktop.install(th, reader=dialog_lines, clicker=lambda ref: verify_ad("click", ref))


def bench_apps():
    return list(getattr(th, "NATIVE_APPS", ["TextEdit", "Preview", "System Settings"]))


def crashed_apps():
    """Bench apps showing a crash-report dialog now. A crash is an infrastructure fault: the run is marked invalid."""
    return sorted({app for app, _ in desktop.crash_dialogs(DESKTOP, bench_apps())})

RUNS = common.taskset_runs(th)


def reset_heldout_desktop():
    """Quit System Settings (only ever read) and the bench documents' apps. An app that shows any
    window that is not a bench window stops the run before the model starts; it is never touched."""
    left = desktop.dismiss_crash_dialogs(DESKTOP, bench_apps())
    if left:
        print(f"dismissed {len(left)} leftover crash dialog(s): {', '.join(sorted(set(left)))}", flush=True)
    for app in bench_apps():
        if app == "System Settings":
            tasks.quit_app(app)
        else:
            th.close_bench_app(app)


def pre_arm_setup(arm, rdir):
    """Infrastructure an arm needs before the model starts; None when ready, else why it is not."""
    if arm == "cua":
        return cua_daemon.start(rdir / "arm-home", str(HERE / "bin/cua/cua-driver"))
    return None


def post_arm_cleanup(arm, rdir):
    """Stop daemons the arm started for this run; they must not carry state into the next run."""
    if arm == "peekaboo":
        env = dict(os.environ, HO_ARM_HOME=str(rdir / "arm-home"))
        subprocess.run([str(HERE / "bin/peekaboo/peekaboo"), "daemon", "stop"], env=env, capture_output=True, timeout=30)
    if arm == "cua":
        env = dict(os.environ, HO_ARM_HOME=str(rdir / "arm-home"))
        for args in (["call", "end_session", "{}"], ["stop"]):
            subprocess.run([str(HERE / "bin/cua/cua-driver"), *args], env=env, capture_output=True, timeout=30)


CAPTURED = re.compile(r'"has_screenshot"\s*:\s*true|"screenshot_raw"\s*:\s*"[^"]+"|"screenshotPath"\s*:\s*"[^"]+"'
                      r'|"command"\s*:\s*"screenshot"\s*,\s*"ok"\s*:\s*true|"ok"\s*:\s*true\s*,\s*"command"\s*:\s*"screenshot"')
GRANT = re.compile(r'"?(accessibility|screen_?recording|screenRecording)"?\s*[:=]\s*\{?\s*(?:"state"\s*:\s*)?"?(\w+)', re.I)
PERMISSION_ARGS = {"agent-desktop": ["permissions"], "peekaboo": ["permissions", "--json"],
                   "cua-driver": ["permissions", "status", "--json"]}


def tool_results(events):
    """The text of every tool result the model saw, in order."""
    for ev in events:
        if ev.get("type") != "user":
            continue
        for block in ev.get("message", {}).get("content", []) or []:
            if isinstance(block, dict) and block.get("type") == "tool_result":
                content = block.get("content")
                yield content if isinstance(content, str) else json.dumps(content)


def captured(events):
    """A capture the tool output shows, or None."""
    for text in tool_results(events):
        m = CAPTURED.search(text)
        if m:
            return m.group(0)[:60]
    return None


def effective_grants(arm, env):
    """What the arm's own tool reports for Accessibility and Screen Recording, through its bench wrapper:
    {"accessibility": bool|None, "screen_recording": bool|None}."""
    tool = common.NATIVE_TOOL[arm]
    try:
        out = subprocess.run([str(common.arm_bin_dir(arm) / tool), *PERMISSION_ARGS[tool]], env=env,
                             capture_output=True, text=True, timeout=30)
        text = out.stdout + out.stderr
    except (OSError, subprocess.TimeoutExpired):
        return {"accessibility": None, "screen_recording": None}
    grants = {}
    for name, value in GRANT.findall(text):
        key = "accessibility" if name.lower() == "accessibility" else "screen_recording"
        grants.setdefault(key, value.lower() in ("true", "granted", "authorized", "yes"))
    return {k: grants.get(k) for k in ("accessibility", "screen_recording")}


def ledger(record):
    """One line per attempt state in the attempt ledger: `started` before the model, then exactly one end state."""
    with open(RUNS / "attempts.jsonl", "a") as fh:
        fh.write(json.dumps(dict(record, t=round(time.time(), 3))) + "\n")


def run_once(a):
    run_id = time.strftime("%m%d%H%M%S") + "-" + uuid.uuid4().hex[:4]
    rdir = RUNS / "native" / run_id
    rdir.mkdir(parents=True)
    cell = {"run_id": run_id, "arm": a.arm, "task": a.task, "round": a.round, "slot": a.slot,
            "experiment": a.experiment}
    ledger(dict(cell, state="started"))
    try:
        row = attempt(a, run_id, rdir)
    except BaseException as exc:
        ledger(dict(cell, state="harness_error", error=f"{type(exc).__name__}: {exc}"[:300]))
        raise
    ledger(dict(cell, state="timed_out" if row.get("timed_out") else "infra_fault" if row.get("not_run")
                else "finished", success=row.get("success"), cost_usd_list=row.get("cost_usd_list")))
    return row


def attempt(a, run_id, rdir):
    task = th.NATIVE_TASKS[a.task](run_id.replace("-", ""), str(common.VERIFY_AD))
    task.round = a.round
    allow_fg = bool(getattr(task, "allows_foreground", False))
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
                "AGENT_DESKTOP_HOME": str(home), "HO_ARM_HOME": str(rdir / "arm-home"),
                "HO_CAPTURE": "on" if a.screen_recording else "off"})
    env.pop("AGENT_DESKTOP_SESSION", None)
    fault = pre_arm_setup(a.arm, rdir)
    grants = effective_grants(a.arm, dict(env, AD_LOG=str(rdir / "probe.log")))
    watch = run_guard.FrontWatch(bench_apps())
    if not fault and watch.start() not in ("other", None):
        fault = "a bench app was frontmost before the model started"
    if fault:
        watch.stop()
        return record_infra_fault(a, task, run_id, rdir, fault)
    cmd = ["claude", "-p", common.TASK_PREFIX + task.prompt(), "--output-format", "stream-json", "--verbose",
           "--setting-sources", "project", "--strict-mcp-config", "--tools", "Bash",
           "--permission-mode", "bypassPermissions", "--model", a.model, "--no-session-persistence",
           "--append-system-prompt", common.native_system_prompt(a.arm, a.screen_recording, allow_fg)]
    t0 = time.time()
    events = []
    child = run_guard.Child(cmd, cwd, env)
    proc = child.proc
    capped = threading.Event()

    def stop_at_cap():
        capped.set()
        child.kill_group()

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
    stderr = child.finish()
    watchdog.cancel()
    wall = time.time() - t0
    front = watch.stop()
    summary = driver.summarize(events, rdir, wall)
    answer = summary.get("answer", "")
    try:
        ok, why = task.verify(answer)
    except Exception as exc:
        ok, why = False, f"verify error {exc}"
    if common.RULES == "v2":
        audit = run_audit.RunAudit(a.arm, th.FORBIDDEN, allow_fg)
        forbidden = [f for f in (audit.check(c) for c in summary["commands"]) if f]
    else:
        forbidden = [f for f in (common.audit_native(c, a.arm, th.FORBIDDEN) for c in summary["commands"]) if f]
    if common.RULES == "v2":
        shown = None if a.screen_recording else captured(events)
        forbidden += [f"capture {shown}"] if shown else []
        fronted = run_guard.foreground_violation(front, allow_fg)
        forbidden += [fronted] if fronted else []
    if forbidden:
        ok, why = False, f"forbidden command {forbidden[0]}; {why}"
    if capped.is_set():
        ok, why = False, f"timed out at the {th.NATIVE_CAP_S} s cap; {why}"
    crashed = [f"{app} during the run" for app in crashed_apps()]
    for step in (task.cleanup, lambda: post_arm_cleanup(a.arm, rdir)):
        try:
            step()
        except Exception as exc:
            print(f"{run_id} cleanup failed: {exc}", flush=True)
    crashed += [f"{app} during cleanup" for app in crashed_apps() if f"{app} during the run" not in crashed]
    desktop.dismiss_crash_dialogs(DESKTOP, bench_apps())
    del summary["commands"]
    summary.update({"suite": "native", "arm": a.arm, "arm_version": common.ARM_VERSIONS[a.arm], "run_id": run_id,
                    "task": a.task, "round": a.round, "model": a.model, "success": ok, "verify": why,
                    "cap_s": th.NATIVE_CAP_S, "timed_out": capped.is_set(), "screen_recording": a.screen_recording,
                    "rules": common.RULES, "violations": forbidden[:5], "grants": grants,
                    "capture": "on" if a.screen_recording else "off", "slot": a.slot, "experiment": a.experiment,
                    "host": a.host, **front})
    summary.update(driver.validity(stderr, rdir))
    if crashed:
        reason = "infrastructure: crash-report dialog for " + ", ".join(crashed)
        summary["valid"] = False
        summary["invalid_reason"] = "; ".join(filter(None, [summary.get("invalid_reason"), reason]))
    (rdir / "summary.json").write_text(json.dumps(summary, indent=1))
    with open(RUNS / "results-native.jsonl", "a") as fh:
        fh.write(json.dumps(summary) + "\n")
    print(f"{run_id} {a.arm:8} {a.task} ok={ok!s:5} wall={wall:6.1f}s cli={summary['cli_s']:5.1f}s "
          f"turns={summary.get('num_turns')} in={summary['input_total']} | {why}"
          + (f" | INVALID {summary['invalid_reason']}" if summary.get("valid") is False else ""), flush=True)
    return summary


def record_infra_fault(a, task, run_id, rdir, fault):
    """An arm that cannot start is the harness's fault, not the tool's: no model call, an invalid row, no spend."""
    for step in (task.cleanup, lambda: post_arm_cleanup(a.arm, rdir)):
        try:
            step()
        except Exception as exc:
            print(f"{run_id} cleanup failed: {exc}", flush=True)
    row = {"suite": "native", "arm": a.arm, "arm_version": common.ARM_VERSIONS[a.arm], "run_id": run_id,
           "task": a.task, "round": a.round, "model": a.model, "success": False, "verify": f"not run: {fault}",
           "cost_usd_list": 0.0, "rules": common.RULES, "host": a.host, "valid": False, "not_run": True,
           "slot": a.slot, "experiment": a.experiment, "invalid_reason": f"infrastructure: {fault}"}
    with open(RUNS / "results-native.jsonl", "a") as fh:
        fh.write(json.dumps(row) + "\n")
    print(f"{run_id} {a.arm:8} {a.task} not run | INVALID infrastructure: {fault}", flush=True)
    return row


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm", required=True, choices=sorted(common.NATIVE_TOOL))
    ap.add_argument("--task", required=True, choices=sorted(th.NATIVE_TASKS))
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--round", type=int, default=0)
    ap.add_argument("--model", default="claude-opus-5-5")
    ap.add_argument("--screen-recording", action="store_true", help="only on a host where every arm has it granted")
    ap.add_argument("--host", default="mac")
    ap.add_argument("--slot", type=int, default=None, help="this cell's index in the saved schedule")
    ap.add_argument("--experiment", default=None, help="the pass name; rows and ledger lines carry it")
    a = ap.parse_args()
    if not driver.holds_lock():
        sys.exit(f"take the bench lock first: {driver.LOCK}")
    RUNS.mkdir(parents=True, exist_ok=True)
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    for _ in range(a.runs):
        run_once(a)


if __name__ == "__main__":
    main()
