#!/usr/bin/env python3
"""Visit public bot-detection pages read-only through each channel and keep the page text.

Channels:
  ax-temp     temp-profile Chrome, no debugging port, page read through agent-desktop (AX)
  cdp-temp    temp-profile Chrome with --remote-debugging-port, read through agent-browser --cdp
  ab-launch   agent-browser launches its own headed Chrome
  ax-real     George's real Chrome profile, launched normally, read through agent-desktop (AX)

Nothing is typed, submitted or logged into. Page text lands in results/raw/detect/ (gitignored)
so the summary can be written from it; listening ports are recorded per channel.
"""
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from lock import release_lock, take_lock  # noqa: E402

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
PAGES = {
    "sannysoft": "https://bot.sannysoft.com/",
    "areyouabot": "https://deviceandbrowserinfo.com/are_you_a_bot",
    "browserscan": "https://www.browserscan.net/bot-detection",
}


def listening_ports():
    out = subprocess.run(["lsof", "-nP", "-iTCP", "-sTCP:LISTEN", "-c", "Google"], capture_output=True, text=True).stdout
    return sorted({line.split()[-2] for line in out.splitlines()[1:] if line.strip()})


def ax_text(ad, home):
    env = dict(os.environ, AGENT_DESKTOP_HOME=home)
    out = subprocess.run([ad, "snapshot", "--app", "Google Chrome", "--max-depth", "60", "--timeout-ms", "20000"],
                         capture_output=True, text=True, env=env, timeout=60).stdout
    texts = []

    def walk(n):
        for k in ("name", "value", "description"):
            if isinstance(n.get(k), str) and n[k].strip():
                texts.append(n[k].strip())
        for c in n.get("children", []):
            walk(c)
    try:
        walk(json.loads(out)["data"]["tree"])
    except Exception:
        return out[:2000]
    return "\n".join(texts)


def ab(bin_dir, args, env):
    return subprocess.run([f"{bin_dir}/agent-browser", *args], capture_output=True, text=True, env=env, timeout=90).stdout


def main():
    bin_dir, out = sys.argv[1], Path(sys.argv[2])
    channels = sys.argv[3].split(",") if len(sys.argv) > 3 else ["ax-temp", "cdp-temp", "ab-launch", "ax-real"]
    raw = out / "raw" / "detect"
    raw.mkdir(parents=True, exist_ok=True)
    ad = f"{bin_dir}/agent-desktop"
    title = "feat/unified-browser detect"
    take_lock(title)
    summary = []
    try:
        for ch in channels:
            work = Path(tempfile.mkdtemp(prefix=f"det-{ch}-", dir=str(out)))
            env = dict(os.environ, AGENT_BROWSER_SOCKET_DIR=tempfile.mkdtemp(prefix="abs."))
            proc = None
            for name, url in PAGES.items():
                if ch in ("ax-temp", "cdp-temp"):
                    args = [CHROME, f"--user-data-dir={work}/profile", "--no-first-run", "--no-default-browser-check"]
                    if ch == "cdp-temp":
                        args.append("--remote-debugging-port=9334")
                    proc = subprocess.Popen(args + [url if ch == "ax-temp" else "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
                elif ch == "ax-real":
                    proc = subprocess.Popen([CHROME, "--no-first-run", url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
                time.sleep(4)
                if ch == "cdp-temp":
                    ab(bin_dir, ["--cdp", "9334", "open", url], env)
                time.sleep(10)
                if ch == "ab-launch":
                    ab(bin_dir, ["--headed", "open", url], env)
                    time.sleep(10)
                ports = listening_ports()
                if ch.startswith("ax"):
                    text = ax_text(ad, str(work / "ad-home"))
                elif ch == "cdp-temp":
                    text = ab(bin_dir, ["--cdp", "9334", "get", "text", "body"], env)
                else:
                    text = ab(bin_dir, ["get", "text", "body"], env)
                (raw / f"{ch}-{name}.txt").write_text(text)
                summary.append({"channel": ch, "page": name, "chars": len(text), "chrome_listening": ports})
                print(json.dumps(summary[-1]), flush=True)
                if ch in ("cdp-temp", "ab-launch"):
                    ab(bin_dir, ["close", "--all"], env)
                if proc:
                    os.killpg(proc.pid, signal.SIGTERM)
                    proc.wait(timeout=15)
                    proc = None
                time.sleep(2)
    finally:
        release_lock(title)
    (out / "detect-summary.json").write_text(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()
