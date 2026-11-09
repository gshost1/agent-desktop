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
import contextlib
import json
import os
import re
import shutil
import socket
import signal
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from tasks import TASKS, real_expectations, verify  # noqa: E402
from lock import release_lock, take_lock  # noqa: E402
from strays import collect_strays  # noqa: E402

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def http_json(url):
    with urllib.request.urlopen(url, timeout=5) as r:
        return json.loads(r.read())


def free_port():
    """A port nothing listens on now; each run gets its own so a Chrome still releasing the last one cannot answer for it."""
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def listener_pids(port):
    out = subprocess.run(["lsof", "-nP", f"-iTCP:{port}", "-sTCP:LISTEN", "-t"], capture_output=True, text=True).stdout
    return {int(p) for p in out.split()}


def endpoint_is_this_runs(port, profile):
    """True when every process listening on `port` runs on `profile`."""
    listening = listener_pids(port)
    return bool(listening) and listening <= set(profile_processes(profile))


def launch_chrome(profile, port, downloads, extra):
    """Start Chrome on a profile folder that did not exist before this call; it is complete before launch."""
    write_profile(profile, downloads)
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
        except Exception:
            time.sleep(0.3)
            continue
        if proc.poll() is None and endpoint_is_this_runs(port, profile):
            return proc
        stop_chrome(proc, profile)
        raise RuntimeError(f"port {port} is answered by a process that is not this run's Chrome")
    stop_chrome(proc, profile)
    raise RuntimeError("chrome devtools endpoint never came up (a startup dialog blocks it)")


class LaunchedChrome:
    """A Chrome that agent-desktop launched through Launch Services: not this process's child, so it is polled by pid."""

    def __init__(self, pid):
        self.pid = pid

    def poll(self):
        try:
            os.kill(self.pid, 0)
            return None
        except ProcessLookupError:
            return 0


def write_profile(profile, downloads):
    profile.mkdir(parents=True)
    (profile / "Default").mkdir()
    prefs = {"download": {"default_directory": str(downloads), "prompt_for_download": False},
             "profile": {"default_content_setting_values": {"automatic_downloads": 1}},
             "browser": {"has_seen_welcome_page": True}}
    staged = profile / "Default" / "Preferences.tmp"
    staged.write_text(json.dumps(prefs))
    os.replace(staged, profile / "Default" / "Preferences")


def ad_launch_command(ad, profile, downloads, extra):
    flags = [f"--user-data-dir={profile}", "--no-first-run", "--no-default-browser-check",
             "--disable-sync", "--password-store=basic", "--use-mock-keychain"] + extra
    return [ad, "launch", "Google Chrome", "--cdp", "--download-dir", str(downloads)] + [f"--arg={f}" for f in flags]


def launch_chrome_ad(ad, home, profile, downloads, extra):
    """The ad-web channel: agent-desktop launches Chrome with its own DevTools endpoint, recorded under `home`."""
    write_profile(profile, downloads)
    env = dict(os.environ, AGENT_DESKTOP_HOME=str(home))
    p = subprocess.run(ad_launch_command(ad, profile, downloads, extra), capture_output=True, text=True, timeout=60, env=env)
    try:
        data = json.loads(p.stdout)
    except ValueError:
        data = {}
    pid = (data.get("data") or {}).get("pid")
    if not data.get("ok") or not pid:
        if profile_processes(profile):
            for pid_left in profile_processes(profile):
                os.kill(pid_left, signal.SIGTERM)
        raise RuntimeError(f"agent-desktop launch --cdp failed: {p.stdout.strip()[:300]} {p.stderr.strip()[:200]}")
    return LaunchedChrome(pid)


def profile_processes(profile):
    out = subprocess.run(["pgrep", "-f", "--", f"--user-data-dir={profile}"], capture_output=True, text=True).stdout
    return [int(p) for p in out.split()]


def devtools_endpoint(profile):
    """(port, browser websocket path) of the DevTools endpoint a process on `profile` was started with, or None."""
    for pid in profile_processes(profile):
        args = subprocess.run(["ps", "-ww", "-o", "args=", "-p", str(pid)], capture_output=True, text=True).stdout
        found = re.search(r"--remote-debugging-port=(\d+)", args)
        if not found:
            continue
        port = int(found.group(1))
        try:
            socket_url = http_json(f"http://127.0.0.1:{port}/json/version")["webSocketDebuggerUrl"]
        except Exception:
            return None
        return port, urllib.parse.urlparse(socket_url).path
    return None


def quit_chrome_gracefully(proc, profile, grace=40):
    """Ask Chrome to quit over DevTools so it removes its code-sign clone, then wait for every profile process."""
    endpoint = devtools_endpoint(profile)
    node = shutil.which("node")
    if endpoint is None or node is None:
        return False
    port, path = endpoint
    try:
        subprocess.run([node, str(HERE / "cdp_close.mjs"), str(port), path], capture_output=True, timeout=15)
    except subprocess.TimeoutExpired:
        return False
    deadline = time.time() + grace
    while time.time() < deadline:
        if proc.poll() is not None and not profile_processes(profile):
            return True
        time.sleep(0.2)
    return False


def stop_chrome(proc, profile):
    """Quit the run's Chrome over DevTools, then kill what is left; True once no process of the profile remains."""
    if quit_chrome_gracefully(proc, profile):
        return True
    for sig, grace in ((signal.SIGTERM, 10), (signal.SIGKILL, 5)):
        try:
            os.killpg(proc.pid, sig)
        except ProcessLookupError:
            pass
        for pid in profile_processes(profile):
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass
        deadline = time.time() + grace
        while time.time() < deadline:
            if proc.poll() is not None and not profile_processes(profile):
                return True
            time.sleep(0.2)
    return proc.poll() is not None and not profile_processes(profile)


def discard_profile(profile):
    """A run's Chrome profile is 100-300 MB of cache and holds nothing a result reads."""
    shutil.rmtree(profile, ignore_errors=True)


def bench_dialogs(ad, app="Google Chrome"):
    """Alerts, sheets and dialogs the bench browser shows. Call only while no other copy of the app runs."""
    try:
        out = subprocess.run([ad, "list-surfaces", "--app", app], capture_output=True, text=True, timeout=15).stdout
        surfaces = json.loads(out).get("data", {}).get("surfaces", [])
    except Exception:
        return []
    return [s.get("title") or s.get("type") for s in surfaces if s.get("type") in ("alert", "sheet", "dialog")]


class BenchDialog(RuntimeError):
    pass


@contextlib.contextmanager
def chrome_session(profile, port, downloads, extra, ad=None, launcher=None):
    """Launch Chrome on a new profile; stop it, wait for every process to exit, then delete the profile.

    With `ad`, a dialog on screen after launch fails the run: the session closes Chrome, so it never stays up.
    """
    proc = None
    try:
        proc = (launcher or launch_chrome)(profile, port, downloads, extra)
        if ad and "--headless=new" not in extra:
            dialogs = bench_dialogs(ad)
            if dialogs:
                raise BenchDialog(f"dialog on screen after launch: {dialogs}")
        yield proc
    finally:
        gone = stop_chrome(proc, profile) if proc is not None else not profile_processes(profile)
        if gone:
            discard_profile(profile)
        else:
            print(f"profile kept, Chrome still running on it: {profile}", flush=True)


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
             "For web pages, learn it with `agent-desktop skills get web` before you start; only if the task "
             "also needs a native app, read `agent-desktop skills get desktop` too. agent-desktop already launched "
             "Chrome with its DevTools endpoint, so do not launch it again.")
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
        if not isinstance(ev, dict) or not isinstance(ev.get("message", {}), dict):
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


def endpoint_port(home):
    try:
        return json.loads((home / "web" / "endpoints.json").read_text())["entries"][0]["port"]
    except (OSError, ValueError, KeyError, IndexError):
        return None


def run_one(args, task, channel, run_idx, out):
    seed = 1000 * (run_idx + 1) + sum(map(ord, task["id"] + channel))
    exp = http_json(f"{args.base}/__reset?seed={seed}")
    try:
        exp.update(real_expectations(task["id"]))
    except Exception as e:
        row = {"task": task["id"], "channel": channel, "run": run_idx, "model": args.model, "ok": False,
               "why": f"expected value unavailable: {e}"}
        with open(out / "runs.jsonl", "a") as f:
            f.write(json.dumps(row) + "\n")
        print(json.dumps(row), flush=True)
        return row
    run_id = f"{task['id']}-{channel}-{run_idx}"
    work = out / "work" / run_id
    shutil.rmtree(work, ignore_errors=True)
    downloads = work / "downloads"
    downloads.mkdir(parents=True)
    kill_apps(task.get("kill", []))
    cdp_port = free_port() if channel in ("cdp", "hybrid") else None
    stack = contextlib.ExitStack()
    try:
        launcher = None
        if channel == "ad-web":
            launcher = lambda profile, port, dl, extra: launch_chrome_ad(args.ad, work / "ad-home", profile, dl, extra)
        chrome = stack.enter_context(chrome_session(work / "profile", cdp_port, downloads, args.chrome_arg, ad=args.ad,
                                                    launcher=launcher))
    except RuntimeError as e:
        row = {"task": task["id"], "channel": channel, "run": run_idx, "model": args.model, "ok": False,
               "why": f"bench launch failed: {e}"}
        with open(out / "runs.jsonl", "a") as f:
            f.write(json.dumps(row) + "\n")
        print(json.dumps(row), flush=True)
        return row
    with stack:
        ctx = {"base": args.base, "work": str(work), "downloads": str(downloads), "exp": exp, "ad": args.ad,
               "cdp_port": cdp_port or endpoint_port(work / "ad-home")}
        prompt = task["prompt"].format(**ctx)
        env = dict(os.environ)
        env["PATH"] = f"{args.bin_dir}:{env['PATH']}"
        env["AGENT_DESKTOP_HOME"] = str(work / "ad-home")
        if args.ab_skills:
            env["AGENT_BROWSER_SKILLS_DIR"] = args.ab_skills
        env["AGENT_BROWSER_SOCKET_DIR"] = tempfile.mkdtemp(prefix="abs.")
        cmd = ["claude", "-p", prompt, "--safe-mode", "--tools", "Bash", "--allowedTools", *allowed_tools(channel),
               "--append-system-prompt", channel_prompt(channel, cdp_port), "--model", args.model,
               "--output-format", "stream-json", "--verbose", "--no-session-persistence"]
        front_before = subprocess.run(["lsappinfo", "front"], capture_output=True, text=True).stdout.strip()
        t0 = time.time()
        since = t0 - 1
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
        strays = collect_strays(since, work, out / "strays" / run_id)
        if strays:
            ok, why = False, f"stray saves moved: {strays}; " + why
        if "--headless=new" not in args.chrome_arg:
            left = bench_dialogs(args.ad)
            if left:
                ok, why = False, f"dialog left on screen: {left}; " + why
        stop_chrome(chrome, work / "profile")
        kill_apps(task.get("kill", []))
        if channel in ("cdp", "hybrid"):
            subprocess.run([f"{args.bin_dir}/agent-browser", "close", "--all"], env=env, capture_output=True, timeout=30)
        u = final.get("usage", {})
        row = {"task": task["id"], "channel": channel, "run": run_idx, "model": args.model, "ok": ok, "why": why,
               "wall_s": round(wall, 1), "timed_out": timed_out, "num_turns": final.get("num_turns"),
               "input_tokens": u.get("input_tokens"), "output_tokens": u.get("output_tokens"),
               "cache_read": u.get("cache_read_input_tokens"), "cache_creation": u.get("cache_creation_input_tokens"),
               "total_input": sum(u.get(k) or 0 for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")),
               "cost_usd_list": final.get("total_cost_usd"), "front_before": front_before, "strays": strays, **s}
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
    ap.add_argument("--bin-dir", required=True, help="folder holding agent-desktop and agent-browser")
    ap.add_argument("--out", default=str(HERE / "results"))
    ap.add_argument("--timeout", type=int, default=900)
    ap.add_argument("--chrome-arg", action="append", default=[])
    ap.add_argument("--lock-title", default="feat/unified-browser bench")
    ap.add_argument("--ab-skills", default="", help="agent-browser skill-data dir from the matching npm package")
    ap.add_argument("--rotate", action="store_true", help="rotate the channel order by run index")
    ap.add_argument("--keep-lock", action="store_true", help="leave the lock held for a follow-up step in the same slot")
    ap.add_argument("--no-lock", action="store_true", help="only for headless Chrome runs that never touch the desktop")
    args = ap.parse_args()
    args.ad = f"{args.bin_dir}/agent-desktop"
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    tasks = [t for t in TASKS if args.tasks == "all" or t["id"] in args.tasks.split(",")]
    for t in [t for t in tasks if t.get("held")]:
        print(f"skipping {t['id']}: {t['held']}", flush=True)
    tasks = [t for t in tasks if not t.get("held")]
    if args.no_lock and "--headless=new" not in args.chrome_arg:
        sys.exit("--no-lock needs --chrome-arg=--headless=new")
    headed = "--headless=new" not in args.chrome_arg
    if headed and subprocess.run(["pgrep", "-x", "Google Chrome"], capture_output=True).returncode == 0:
        sys.exit("Google Chrome is already running: a headed run would drive or inspect the user's browser")
    if not args.no_lock:
        take_lock(args.lock_title)
    try:
        for r in range(args.start_run, args.start_run + args.runs):
            for t in tasks:
                order = args.channels.split(",")
                shift = r % len(order) if args.rotate else 0
                for ch in order[shift:] + order[:shift]:
                    if ch not in t["channels"]:
                        continue
                    run_one(args, t, ch, r, out)
    finally:
        if not args.no_lock and not args.keep_lock:
            release_lock(args.lock_title)


if __name__ == "__main__":
    main()
