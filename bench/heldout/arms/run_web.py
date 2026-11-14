#!/usr/bin/env python3
"""Held-out web run: feat-unified-browser/bench/browser/run.py with the held-out tasks and one arm.

Nothing in the browser bench is edited; its task list, verifier, prompts and tool allow-list are
replaced in memory, the way runs/1110-evening/run_web_bh.py plugs in browser-harness:
  ad-up  run.py channel `ax`     upstream agent-desktop, accessibility tree only
  ad-14  run.py channel `ad-web` agent-desktop 1.4.0 with its web channel
  bh     run.py channel `cdp`    browser-harness on the run's Chrome DevTools port
  pw     run.py channel `cdp`    playwright-cli attached to the run's Chrome DevTools port
  ab     run.py channel `cdp`    agent-browser 0.42.0 with run.py's own agent-browser prompt (`--cdp <port>`)
Every arm sees the same data per task and run index (tasks_heldout.web_seed). Each run's tool
calls are timed by the arm's wrapper into <work>/tool.log; one row per run goes to
heldout/runs/results-web.jsonl.

Start heldout/web/server.py first (port 8792). Google Chrome must not be running.

Usage: python3.11 -I heldout/arms/run_web.py --arm ad-14 --tasks hw1,hw2 --runs 1 --start-run 0
       [--model claude-opus-5-5] [--timeout 600]
"""
import argparse
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import common  # noqa: E402
import tasks_heldout as th  # noqa: E402

sys.path.insert(0, str(common.BROWSER_BENCH))
import run  # noqa: E402

PORT_FILE = common.HELDOUT / "out" / "web-current-port"
RESULTS = common.HELDOUT / "runs" / "results-web.jsonl"
ORIGINAL = {"prompt": run.channel_prompt, "tools": run.allowed_tools, "http": run.http_json, "run_one": run.run_one,
            "dialogs": run.bench_dialogs, "launch_cmd": run.ad_launch_command,
            "stop": run.stop_chrome}
CURRENT = {}
COMMON = (" Google Chrome is already open. Do not use any other program (no curl, osascript, open, python). "
          "When finished, reply with one line: DONE <answer if the task asks for one>.")


def channel_prompt(channel, cdp_port):
    arm = CURRENT["arm"]
    if arm == "bh":
        PORT_FILE.write_text(str(cdp_port))
        return ("You operate Google Chrome only through the `browser-harness` CLI (on PATH), which drives Chrome over "
                "CDP. Chrome is already running and browser-harness is already connected to it. Learn it with "
                "`browser-harness skill` before you start." + COMMON)
    if arm == "pw":
        PORT_FILE.write_text(str(cdp_port))
        return ("You operate Google Chrome only through the `playwright-cli` CLI (on PATH). Chrome is already running "
                f"with a DevTools endpoint at http://127.0.0.1:{cdp_port}; connect to it with "
                f"`playwright-cli attach --cdp=http://127.0.0.1:{cdp_port}` and do not open another browser. Learn it "
                f"from its skill before you start: `cat {common.SKILLS}/playwright-cli/SKILL.md` (its references are "
                "in the same folder)." + COMMON)
    return ORIGINAL["prompt"](channel, cdp_port)


def allowed_tools(channel):
    tool = common.WEB_TOOL[CURRENT["arm"]]
    base = [t for t in ORIGINAL["tools"](channel) if not t.startswith(("Bash(agent-browser", "Bash(agent-desktop"))]
    return base + ["Bash(cat:*)", f"Bash({tool}:*)"]


def http_json(url):
    if "/__reset?" in url and "seed" in CURRENT:
        url = re.sub(r"seed=\d+", f"seed={CURRENT['seed']}", url)
    return ORIGINAL["http"](url)


def bench_dialogs(ad, app="Google Chrome"):
    return ORIGINAL["dialogs"](str(common.VERIFY_AD), app)


def ad_launch_command(ad, profile, downloads, extra):
    """The harness launches the ad-web Chrome with the real 1.4.0 binary, so setup is never logged as tool time."""
    return ORIGINAL["launch_cmd"](str(common.VERIFY_AD), profile, downloads, extra)


def stop_chrome(chrome, profile):
    """Marks the end of the model's tool calls. run.py's `agent-browser close --all` cleanup runs after this,
    and for the ab arm that command is the logged tool itself, so it must not count as tool time. The first
    stop wins: chrome_session stops Chrome again on exit, after that cleanup."""
    log = Path(profile).parent / "tool.log"
    if "cut" not in CURRENT:
        CURRENT["cut"] = len(log.read_text(errors="replace").splitlines()) if log.exists() else 0
    return ORIGINAL["stop"](chrome, profile)


def tool_log_totals(path, limit=None):
    calls, seconds = 0, 0.0
    if path.exists():
        for line in path.read_text(errors="replace").splitlines()[:limit]:
            parts = line.split("\t")
            if len(parts) >= 5:
                calls += 1
                seconds += float(parts[1]) - float(parts[0])
    return calls, round(seconds, 3)


def run_one(args, task, channel, run_idx, out):
    CURRENT["seed"] = th.web_seed(task["id"], run_idx)
    CURRENT.pop("cut", None)
    row = ORIGINAL["run_one"](args, task, channel, run_idx, out)
    calls, seconds = tool_log_totals(out / "work" / f"{task['id']}-{channel}-{run_idx}" / "tool.log",
                                     CURRENT.pop("cut", None))
    why = row.get("why", "")
    invalid = why.startswith(("expected value unavailable", "bench launch failed"))
    record = {"suite": "web", "arm": CURRENT["arm"], "arm_version": common.ARM_VERSIONS[CURRENT["arm"]],
              "task": task["id"], "round": run_idx, "model": args.model, "channel": channel, "seed": CURRENT["seed"],
              "success": bool(row.get("ok")), "verify": why, "wall_s": row.get("wall_s"), "cli_s": seconds,
              "cli_calls": calls, "num_turns": row.get("num_turns"), "input_total": row.get("total_input"),
              "output_tokens": row.get("output_tokens"), "tool_calls": row.get("tool_calls"),
              "timed_out": row.get("timed_out"), "valid": not invalid, "host": CURRENT["host"]}
    if invalid:
        record["invalid_reason"] = why
    RESULTS.parent.mkdir(parents=True, exist_ok=True)
    with open(RESULTS, "a") as fh:
        fh.write(json.dumps(record) + "\n")
    return row


def main():
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument("--arm", required=True, choices=sorted(common.WEB_ARMS))
    ap.add_argument("--base", default="http://127.0.0.1:8792")
    ap.add_argument("--model", default="claude-opus-5-5")
    ap.add_argument("--host", default="mac")
    a, rest = ap.parse_known_args()
    CURRENT.update({"arm": a.arm, "host": a.host})
    for name in ("channel_prompt", "allowed_tools", "http_json", "run_one", "bench_dialogs", "ad_launch_command",
                 "stop_chrome"):
        setattr(run, name, globals()[name])
    run.TASKS = th.WEB_TASKS
    run.verify = th.verify_web
    run.real_expectations = th.web_expectations
    out = common.HELDOUT / "runs" / "web" / f"{a.arm}-{a.model}"
    sys.argv = [sys.argv[0], *rest, "--channels", common.WEB_ARMS[a.arm], "--bin-dir", str(common.arm_bin_dir(a.arm)),
                "--base", a.base, "--model", a.model, "--out", str(out), "--lock-title", "perf/speed-cost heldout web"]
    run.main()


if __name__ == "__main__":
    main()
