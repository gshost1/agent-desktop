#!/usr/bin/env python3
"""Where a held-out run's time and tokens went: per run, the tool calls grouped by subcommand.

Native rows carry their per-call log (`cli`: args, seconds, rc, bytes). Web rows do not; their calls are read
from the run's tool.log under <runs>/web/<arm>-<model>/work/<task>-<channel>-<round>/ (a copy of the remote
heldout/runs folder).

Usage: python3 -I heldout/explain.py --task hp3 --arms ad-main,ab --results <jsonl>... [--runs <runs dir>]
"""
import argparse
import json
import shlex
from collections import defaultdict
from pathlib import Path


def native_calls(row):
    return [{"args": c.get("args", ""), "s": float(c.get("s") or 0), "rc": c.get("rc", 0),
             "bytes": int(c.get("bytes") or 0)} for c in row.get("cli") or []]


def web_calls(row, runs):
    if not runs:
        return []
    log = (Path(runs) / "web" / f"{row['arm']}-{row['model']}" / "work" /
           f"{row['task']}-{row.get('channel', 'ad-web')}-{row.get('round')}" / "tool.log")
    out = []
    if log.exists():
        for line in log.read_text(errors="replace").splitlines():
            parts = line.split("\t", 4)
            if len(parts) == 5:
                out.append({"args": parts[4], "s": float(parts[1]) - float(parts[0]), "rc": int(parts[2] or 0),
                            "bytes": int(parts[3] or 0)})
    return out


def subcommand(args):
    try:
        words = shlex.split(args)
    except ValueError:
        words = args.split()
    words = [w for w in words if not w.startswith("-")]
    return " ".join(words[:2]) if words[:1] == ["skills"] else (words[0] if words else "?")


def summarize(row, runs=None):
    calls = native_calls(row) if row.get("suite") == "native" else web_calls(row, runs)
    by = defaultdict(lambda: [0, 0.0, 0])
    for c in calls:
        k = subcommand(c["args"])
        by[k][0] += 1
        by[k][1] += c["s"]
        by[k][2] += c["bytes"]
    slow = max(calls, key=lambda c: c["s"], default=None)
    failed = [c for c in calls if c["rc"]]
    return {"calls": len(calls), "by": dict(by), "slowest": slow, "failed": failed}


def render(rows, runs):
    lines = []
    for r in rows:
        s = summarize(r, runs)
        tokens = int(r.get("input_total") or 0) + int(r.get("output_tokens") or 0)
        top = sorted(s["by"].items(), key=lambda kv: -kv[1][1])[:6]
        lines.append(f"- {r['arm']} {r['task']} r{r.get('round')} ok={r.get('success')} wall={r.get('wall_s')} "
                     f"cli={r.get('cli_s')} turns={r.get('num_turns')} tokens={tokens // 1000}k calls={s['calls']} "
                     f"failed={len(s['failed'])} | " +
                     ", ".join(f"{k} x{n} {t:.1f}s {b // 1000}kB" for k, (n, t, b) in top) +
                     (f" | slowest {s['slowest']['s']:.2f}s `{s['slowest']['args'][:80]}`" if s["slowest"] else "") +
                     f" | {str(r.get('verify', ''))[:100]}")
    return lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--task", required=True)
    ap.add_argument("--arms", required=True)
    ap.add_argument("--results", nargs="+", required=True)
    ap.add_argument("--runs")
    a = ap.parse_args()
    arms = a.arms.split(",")
    rows = [json.loads(line) for p in a.results for line in Path(p).read_text().splitlines() if line.strip()]
    rows = [r for r in rows if r.get("task") == a.task and r.get("arm") in arms and r.get("valid") is not False]
    rows.sort(key=lambda r: (arms.index(r["arm"]), r.get("round") or 0))
    print("\n".join(render(rows, a.runs)))


if __name__ == "__main__":
    main()
