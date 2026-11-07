#!/usr/bin/env python3
"""Calibrate bytes -> tokens for agent-desktop output with real `claude -p` usage.

Sends one prompt per sample file wrapping the payload, and one with an empty
payload. tokens(sample) = total input tokens (input + cache create + cache read)
minus the empty-payload total. Subscription billing via the logged-in CLI.

Usage: python3 bench/token_calibrate.py [--model claude-opus-5-5] FILE...
Writes bench/out/token_calibration.json.
"""
import argparse
import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PROMPT = "Do not use tools. Reply with only the word OK.\n<data>\n{payload}\n</data>"


def total_input(model, payload):
    cwd = tempfile.mkdtemp(prefix="ad-tokcal-")
    proc = subprocess.run(
        ["claude", "-p", PROMPT.format(payload=payload), "--output-format", "json",
         "--setting-sources", "project", "--strict-mcp-config", "--tools", "",
         "--model", model, "--no-session-persistence"],
        capture_output=True, text=True, cwd=cwd, timeout=300)
    u = json.loads(proc.stdout)["usage"]
    return u["input_tokens"] + u["cache_creation_input_tokens"] + u["cache_read_input_tokens"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="claude-opus-5-5")
    ap.add_argument("files", nargs="+")
    a = ap.parse_args()
    base = total_input(a.model, "")
    rows = []
    for f in a.files:
        text = Path(f).read_text()
        tok = total_input(a.model, text) - base
        b = len(text.encode())
        rows.append({"file": Path(f).name, "bytes": b, "tokens": tok, "bytes_per_token": round(b / tok, 2)})
        print(f"{Path(f).name:50} {b:7d} B {tok:6d} tok {b/tok:5.2f} B/tok", flush=True)
    tb = sum(r["bytes"] for r in rows)
    tt = sum(r["tokens"] for r in rows)
    print(f"{'TOTAL':50} {tb:7d} B {tt:6d} tok {tb/tt:5.2f} B/tok (baseline {base})")
    (ROOT / "out").mkdir(exist_ok=True)
    (ROOT / "out" / "token_calibration.json").write_text(json.dumps(
        {"model": a.model, "baseline_tokens": base, "rows": rows, "bytes_per_token": round(tb / tt, 3)}, indent=1))


if __name__ == "__main__":
    main()
