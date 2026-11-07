#!/usr/bin/env python3
"""Thin model-loop driver: one benchmark task, one config, one `claude -p` run.

The agent gets Bash only, the shipped agent-desktop skill as an appended system
prompt (plus a config addendum), and a PATH whose `agent-desktop` is a wrapper
that logs CLI wall time and output size per call. Billing is the logged-in
subscription (`claude -p`, no API key).

Per run it writes bench/runs/<run_id>/ (gitignored: transcripts hold desktop
content): stream.jsonl (timestamped events), ad.log (CLI calls), summary.json.
Every summary line is appended to bench/runs/results.jsonl.

Usage: python3 bench/driver.py --task weather --config a [--model claude-opus-5-5] [--runs 3]
"""
import argparse
import json
import os
import subprocess
import sys
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent
sys.path.insert(0, str(ROOT))
from tasks import TASKS, audit_command, reset_desktop  # noqa: E402

REAL_BIN = REPO / "target" / "release" / "agent-desktop"
SKILL = REPO / "skills" / "agent-desktop" / "SKILL.md"
TREE = "bench"


def use_tree(tree):
    """Point the run at another checkout's release binary and shipped skill."""
    global REAL_BIN, SKILL, TREE
    root = Path(tree).resolve()
    REAL_BIN = root / "target" / "release" / "agent-desktop"
    SKILL = root / "skills" / "agent-desktop" / "SKILL.md"
    TREE = root.name
    if not REAL_BIN.exists():
        sys.exit(f"no release binary in {root}")
CONFIGS = {"a": None, "b": "b-lean.md", "c": "c-batch.md", "d": "d-no-resnapshot.md", "e": "e-combined.md",
           "m": None, "mx": None, "x": None}
MCP_CONFIGS = {"m", "mx"}
NO_SKILL = {"mx", "x"}
LOCK = Path.home() / ".agent-desktop" / "bench.lock"
RUN_TIMEOUT_S = 900

MCP_RULES = (
    "You operate this Mac's GUI through the agent-desktop MCP tools, with accessibility granted. "
    "Screen Recording is off, so screenshots fail. When the task is complete, reply with the final "
    "answer only.\n\nTask: "
)
RULES = (
    "You operate this Mac's GUI through the `agent-desktop` CLI, which is on PATH with accessibility "
    "granted. Use Bash only to run agent-desktop; piping its output through jq, grep or head is fine. "
    "Do not use any other program to do the task or to read the answer (no open, osascript, mkdir, "
    "touch, defaults, sw_vers, sysctl, ls, cat, python). Screen Recording is off, so screenshots fail. "
    "When the task is complete, reply with the final answer only.\n\nTask: "
)


def system_prompt(config):
    if config in NO_SKILL:
        return "The agent-desktop CLI is installed. Run `agent-desktop --help` to learn it."
    text = SKILL.read_text()
    if CONFIGS[config]:
        text += "\n" + (ROOT / "configs" / CONFIGS[config]).read_text()
    return text


def classify(cmd):
    if "agent-desktop" not in cmd:
        return "other"
    for key in ["batch", "snapshot", "find", "skills", "get", "wait", "launch", "list-", "status", "screenshot"]:
        if f"agent-desktop {key}" in cmd or f"agent-desktop --headed {key}" in cmd:
            return key.rstrip("-")
    return "action"


def run_once(task_key, config, model, effort):
    run_id = time.strftime("%m%d%H%M%S") + "-" + uuid.uuid4().hex[:4]
    rdir = ROOT / "runs" / run_id
    rdir.mkdir(parents=True)
    task = TASKS[task_key](run_id.replace("-", ""), str(REAL_BIN))
    reset_desktop()
    time.sleep(1.5)
    task.setup()
    cwd = rdir / "cwd"
    cwd.mkdir()
    home = rdir / "ad-home"
    home.mkdir(mode=0o700)
    env = dict(os.environ)
    env.update({
        "PATH": f"{ROOT / 'bin'}:{env['PATH']}",
        "AD_REAL": str(REAL_BIN),
        "AD_LOG": str(rdir / "ad.log"),
        "AGENT_DESKTOP_HOME": str(home),
    })
    env.pop("AGENT_DESKTOP_SESSION", None)
    if config in MCP_CONFIGS:
        mcp = rdir / "mcp.json"
        mcp.write_text(json.dumps({"mcpServers": {"agent-desktop": {"command": str(REAL_BIN), "args": ["mcp"],
                                                                    "env": {"AGENT_DESKTOP_HOME": str(home)}}}}))
        cmd = ["claude", "-p", MCP_RULES + task.prompt(), "--output-format", "stream-json", "--verbose",
               "--setting-sources", "project", "--strict-mcp-config", "--mcp-config", str(mcp), "--tools", "",
               "--permission-mode", "bypassPermissions", "--model", model, "--no-session-persistence"]
        if config == "m":
            cmd += ["--append-system-prompt", SKILL.read_text()]
    else:
        cmd = ["claude", "-p", RULES + task.prompt(), "--output-format", "stream-json", "--verbose",
               "--setting-sources", "project", "--strict-mcp-config", "--tools", "Bash",
               "--permission-mode", "bypassPermissions", "--model", model, "--no-session-persistence",
               "--append-system-prompt", system_prompt(config)]
    if effort:
        cmd += ["--effort", effort]
    t0 = time.time()
    events = []
    proc = subprocess.Popen(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            stdin=subprocess.DEVNULL, text=True)
    timed_out = False
    with open(rdir / "stream.jsonl", "w") as fh:
        for line in proc.stdout:
            t = time.time() - t0
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            ev["_t"] = round(t, 3)
            events.append(ev)
            fh.write(json.dumps(ev) + "\n")
            if t > RUN_TIMEOUT_S:
                proc.kill()
                timed_out = True
                break
    proc.wait()
    wall = time.time() - t0
    summary = summarize(events, rdir, wall)
    answer = summary.get("answer", "")
    try:
        ok, why = task.verify(answer)
    except Exception as exc:
        ok, why = False, f"verify error {exc}"
    forbidden = [f for f in (audit_command(c) for c in summary["commands"]) if f]
    if forbidden:
        ok, why = False, f"forbidden command {forbidden[0]}; {why}"
    if timed_out:
        ok, why = False, f"timed out; {why}"
    task.cleanup()
    summary.update({"run_id": run_id, "task": task_key, "config": config, "model": model, "tree": TREE,
                    "effort": effort or "default", "success": ok, "verify": why})
    del summary["commands"]
    (rdir / "summary.json").write_text(json.dumps(summary, indent=1))
    with open(ROOT / "runs" / "results.jsonl", "a") as fh:
        fh.write(json.dumps(summary) + "\n")
    print(f"{run_id} {task_key:10} cfg={config} {model:16} ok={ok!s:5} wall={wall:6.1f}s "
          f"calls={summary['api_calls']:3} in={summary['input_total']:8} out={summary['output_tokens']:6} "
          f"cli={summary['cli_s']:5.1f}s | {why}", flush=True)
    return summary


def summarize(events, rdir, wall):
    calls, order = {}, []
    tool_uses, tool_results = {}, {}
    last_call_t = {}
    result = {}
    for ev in events:
        if ev.get("type") == "assistant":
            msg = ev["message"]
            mid = msg["id"]
            if mid not in calls:
                order.append(mid)
            calls[mid] = msg.get("usage", {})
            last_call_t[mid] = ev["_t"]
            for block in msg.get("content", []):
                if block.get("type") == "tool_use":
                    name = block.get("name", "")
                    cmd = block["input"].get("command", "") if name == "Bash" else \
                        "agent-desktop " + name.split("__")[-1] + " " + json.dumps(block["input"])
                    tool_uses[block["id"]] = {"call": mid, "cmd": cmd, "t": ev["_t"]}
        elif ev.get("type") == "user":
            for block in ev.get("message", {}).get("content", []) or []:
                if isinstance(block, dict) and block.get("type") == "tool_result":
                    content = block.get("content")
                    text = content if isinstance(content, str) else json.dumps(content)
                    tool_results[block["tool_use_id"]] = {"bytes": len(text.encode()), "t": ev["_t"]}
        elif ev.get("type") == "result":
            result = ev
    steps = []
    for i, mid in enumerate(order):
        u = calls[mid]
        steps.append({
            "call": i,
            "input_total": u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0),
            "cache_read": u.get("cache_read_input_tokens", 0),
            "output": u.get("output_tokens", 0),
            "t_end": last_call_t[mid],
            "tools": [],
        })
    index = {mid: i for i, mid in enumerate(order)}
    for tid, tu in tool_uses.items():
        tr = tool_results.get(tid, {})
        steps[index[tu["call"]]]["tools"].append({
            "cmd": tu["cmd"][:300], "class": classify(tu["cmd"]),
            "result_bytes": tr.get("bytes", 0), "tool_s": round(tr.get("t", tu["t"]) - tu["t"], 3)})
    cli = []
    log = rdir / "ad.log"
    if log.exists():
        for line in log.read_text(errors="replace").splitlines():
            parts = line.split("\t")
            if len(parts) >= 5:
                cli.append({"s": float(parts[1]) - float(parts[0]), "rc": int(parts[2]), "bytes": int(parts[3]), "args": parts[4][:200]})
    u = result.get("usage", {})
    tool_s = round(sum(t["tool_s"] for s in steps for t in s["tools"]), 3)
    return {
        "answer": result.get("result", ""),
        "wall_s": round(wall, 2),
        "duration_ms": result.get("duration_ms"),
        "duration_api_ms": result.get("duration_api_ms"),
        "num_turns": result.get("num_turns"),
        "api_calls": len(steps),
        "input_total": sum(s["input_total"] for s in steps),
        "cache_read": sum(s["cache_read"] for s in steps),
        "output_tokens": sum(s["output"] for s in steps),
        "result_usage": u,
        "cost_usd_list": result.get("total_cost_usd"),
        "cli_calls": len(cli),
        "cli_s": round(sum(c["s"] for c in cli), 3) if cli else tool_s,
        "cli_bytes": sum(c["bytes"] for c in cli),
        "tool_s": tool_s,
        "bash_calls": sum(len(s["tools"]) for s in steps),
        "steps": steps,
        "cli": cli,
        "commands": [tu["cmd"] for tu in tool_uses.values()],
    }


def holds_lock():
    return LOCK.exists() and "perf/speed-cost" in LOCK.read_text()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--task", required=True, choices=sorted(TASKS))
    ap.add_argument("--config", required=True, choices=sorted(CONFIGS))
    ap.add_argument("--model", default="claude-opus-5-5")
    ap.add_argument("--effort", default=None)
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--tree", default=None, help="checkout whose release binary and skill to use")
    a = ap.parse_args()
    if a.tree:
        use_tree(a.tree)
    if not holds_lock():
        sys.exit(f"take the bench lock first: {LOCK}")
    (ROOT / "runs").mkdir(exist_ok=True)
    for _ in range(a.runs):
        run_once(a.task, a.config, a.model, a.effort)


if __name__ == "__main__":
    main()
