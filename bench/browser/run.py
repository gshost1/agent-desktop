#!/usr/bin/env python3
"""Run browser and cross-app tasks through a headless `claude -p` driver.

One run = one task through one channel:
  ax      agent-desktop only (accessibility tree; Chrome has no debugging port)
  cdp     agent-browser only, attached to the harness-launched Chrome over CDP
  hybrid  both tools in one loop
  ad-web  agent-desktop with its own web channel (the build under test)

Each run gets a fresh Chrome profile in a temp folder, a fresh seed on the local
test server, and is judged by independent observation (server log, file on
disk, Calculator's display read after the run), never by the agent's claim.
Raw transcripts go to results/raw/ (gitignored).
"""
import argparse
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from tasks import TASKS, verify  # noqa: E402
from lock import release_lock, take_lock  # noqa: E402

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def http_json(url):
    with urllib.request.urlopen(url, timeout=5) as r:
        return json.loads(r.read())


def launch_chrome(profile, port, downloads, extra):
    profile.mkdir(parents=True, exist_ok=True)
    (profile / "Default").mkdir(exist_ok=True)
    prefs = {"download": {"default_directory": str(downloads), "prompt_for_download": False},
             "profile": {"default_content_setting_values": {"automatic_downloads": 1}},
             "browser": {"has_seen_welcome_page": True}}
    (profile / "Default" / "Preferences").write_text(json.dumps(prefs))
    args = [CHROME, f"--user-data-dir={profile}", "--no-first-run", "--no-default-browser-check",
            "--disable-sync", "--password-store=basic", "--use-mock-keychain"] + extra
    if port:
        args.append(f"--remote-debugging-port={port}")
    args.append("about:blank")
    proc = subprocess.Popen(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    deadline = time.time() + 20
    while time.time() < deadline:
        if not port:
            time.sleep(3)
            return proc
        try:
            http_json(f"http://127.0.0.1:{port}/json/version")
            return proc
        except Exception:
            time.sleep(0.3)
    raise RuntimeError("chrome devtools endpoint never came up")


def stop_chrome(proc):
    try:
        os.killpg(proc.pid, signal.SIGTERM)
        proc.wait(timeout=10)
    except Exception:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except Exception:
            pass


def kill_apps(names):
    for n in names:
        subprocess.run(["pkill", "-x", n], capture_output=True)
    time.sleep(1)


def channel_prompt(channel, cdp_port):
    ad = ("You operate this Mac only through the `agent-desktop` CLI (on PATH). "
          "It reads the macOS accessibility tree and acts on refs. Learn it with `agent-desktop skills get desktop` "
          "(and `agent-desktop skills get platform`) before you start.")
    ab = (f"You operate Google Chrome only through the `agent-browser` CLI (on PATH), which drives Chrome over CDP. "
          f"Chrome is already running with a DevTools endpoint on port {cdp_port}; pass `--cdp {cdp_port}` on every "
          "agent-browser command. Learn it with `agent-browser skills get core` before you start.")
    adweb = ("You operate this Mac and its web pages only through the `agent-desktop` CLI (on PATH). "
             "Learn it with `agent-desktop skills get desktop` (and `agent-desktop skills get platform`) before you start.")
    common = (" Google Chrome is already open. Do not use any other program (no curl, osascript, open, python). "
              "When finished, reply with one line: DONE <answer if the task asks for one>.")
    return {"ax": ad, "cdp": ab, "hybrid": ab + " For native macOS apps use the `agent-desktop` CLI (on PATH): " + ad,
            "ad-web": adweb}[channel] + common


def allowed_tools(channel):
    base = ["Bash(jq:*)", "Bash(grep:*)", "Bash(head:*)", "Bash(tail:*)", "Bash(sleep:*)", "Bash(wc:*)"]
    if channel in ("ax", "hybrid", "ad-web"):
        base.append("Bash(agent-desktop:*)")
    if channel in ("cdp", "hybrid"):
        base.append("Bash(agent-browser:*)")
    return base


def summarize_stream(lines):
    turns, tool_calls, tool_chars, snap_chars, snap_calls = 0, 0, 0, 0, 0
    pending = {}
    final = {}
    for line in lines:
        try:
            ev = json.loads(line)
        except Exception:
            continue
        if ev.get("type") == "assistant":
            turns += 1
            for c in ev.get("message", {}).get("content", []):
                if c.get("type") == "tool_use":
                    tool_calls += 1
                    pending[c["id"]] = c.get("input", {}).get("command", "")
        elif ev.get("type") == "user":
            for c in ev.get("message", {}).get("content", []) if isinstance(ev.get("message", {}).get("content"), list) else []:
                if c.get("type") == "tool_result":
                    content = c.get("content")
                    text = content if isinstance(content, str) else "".join(x.get("text", "") for x in content or [])
                    tool_chars += len(text)
                    cmd = pending.get(c.get("tool_use_id"), "")
                    if " snapshot" in cmd or " find" in cmd:
                        snap_chars += len(text)
                        snap_calls += 1
        elif ev.get("type") == "result":
            final = ev
    return {"assistant_messages": turns, "tool_calls": tool_calls, "tool_result_tokens_est": tool_chars // 4,
            "snapshot_tokens_est": snap_chars // 4, "snapshot_calls": snap_calls, "final": final}


def run_one(args, task, channel, run_idx, out):
    seed = 1000 * (run_idx + 1) + sum(map(ord, task["id"] + channel))
    exp = http_json(f"{args.base}/__reset?seed={seed}")
    run_id = f"{task['id']}-{channel}-{run_idx}"
    work = out / "work" / run_id
    shutil.rmtree(work, ignore_errors=True)
    downloads = work / "downloads"
    downloads.mkdir(parents=True)
    kill_apps(task.get("kill", []))
    cdp_port = args.cdp_port if channel in ("cdp", "hybrid") else None
    chrome = launch_chrome(work / "profile", cdp_port, downloads, args.chrome_arg)
    ctx = {"base": args.base, "work": str(work), "downloads": str(downloads), "exp": exp, "ad": args.ad}
    prompt = task["prompt"].format(**ctx)
    env = dict(os.environ)
    env["PATH"] = f"{args.bin_dir}:{env['PATH']}"
    env["AGENT_DESKTOP_HOME"] = str(work / "ad-home")
    env["AGENT_BROWSER_SOCKET_DIR"] = tempfile.mkdtemp(prefix="abs.")
    cmd = ["claude", "-p", prompt, "--safe-mode", "--tools", "Bash", "--allowedTools", *allowed_tools(channel),
           "--append-system-prompt", channel_prompt(channel, cdp_port), "--model", args.model,
           "--output-format", "stream-json", "--verbose", "--no-session-persistence"]
    front_before = subprocess.run(["lsappinfo", "front"], capture_output=True, text=True).stdout.strip()
    t0 = time.time()
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=args.timeout, cwd=str(work), env=env)
        stdout, timed_out = p.stdout, False
    except subprocess.TimeoutExpired as e:
        stdout, timed_out = (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or ""), True
    wall = time.time() - t0
    raw = out / "raw" / f"{run_id}.jsonl"
    raw.parent.mkdir(parents=True, exist_ok=True)
    raw.write_text(stdout)
    s = summarize_stream(stdout.splitlines())
    final = s.pop("final")
    answer = final.get("result", "") or ""
    state = http_json(f"{args.base}/__state")
    ok, why = verify(task, ctx, answer, state)
    stop_chrome(chrome)
    kill_apps(task.get("kill", []))
    if channel in ("cdp", "hybrid"):
        subprocess.run([f"{args.bin_dir}/agent-browser", "close", "--all"], env=env, capture_output=True, timeout=30)
    u = final.get("usage", {})
    row = {"task": task["id"], "channel": channel, "run": run_idx, "model": args.model, "ok": ok, "why": why,
           "wall_s": round(wall, 1), "timed_out": timed_out, "num_turns": final.get("num_turns"),
           "input_tokens": u.get("input_tokens"), "output_tokens": u.get("output_tokens"),
           "cache_read": u.get("cache_read_input_tokens"), "cache_creation": u.get("cache_creation_input_tokens"),
           "total_input": sum(u.get(k) or 0 for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")),
           "cost_usd_list": final.get("total_cost_usd"), "front_before": front_before, **s}
    with open(out / "runs.jsonl", "a") as f:
        f.write(json.dumps(row) + "\n")
    print(json.dumps({k: row[k] for k in ("task", "channel", "run", "ok", "why", "wall_s", "num_turns", "total_input", "output_tokens", "snapshot_tokens_est")}), flush=True)
    return row


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tasks", default="all")
    ap.add_argument("--channels", default="ax,cdp,hybrid")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--start-run", type=int, default=0)
    ap.add_argument("--model", default="sonnet")
    ap.add_argument("--base", default="http://127.0.0.1:8791")
    ap.add_argument("--cdp-port", type=int, default=9333)
    ap.add_argument("--bin-dir", required=True, help="folder holding agent-desktop and agent-browser")
    ap.add_argument("--out", default=str(HERE / "results"))
    ap.add_argument("--timeout", type=int, default=900)
    ap.add_argument("--chrome-arg", action="append", default=[])
    ap.add_argument("--lock-title", default="feat/unified-browser bench")
    args = ap.parse_args()
    args.ad = f"{args.bin_dir}/agent-desktop"
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    tasks = [t for t in TASKS if args.tasks == "all" or t["id"] in args.tasks.split(",")]
    take_lock(args.lock_title)
    try:
        for r in range(args.start_run, args.start_run + args.runs):
            for t in tasks:
                for ch in args.channels.split(","):
                    if ch not in t["channels"]:
                        continue
                    run_one(args, t, ch, r, out)
    finally:
        release_lock(args.lock_title)


if __name__ == "__main__":
    main()
