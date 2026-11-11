#!/usr/bin/env python3
"""Held-out benchmark report from the results jsonl files, as Markdown on stdout.

Order of every table: CLI/tool time, wall time, tokens, turns, pass rate. Per task and arm it
gives min / median / max over the valid runs (passes and failures alike), and it lists every
task and metric where agent-desktop 1.4 (arm ad-14) is worse than another arm.

  CLI s    seconds inside the arm's tool, summed over the run (wrapper log, arms/logwrap.zsh)
  wall s   whole run, claude -p start to exit
  tokens   input (uncached + cache read + cache write) plus output, summed over the run
  turns    claude -p num_turns
  pass     verifier passes / valid runs

Usage: python3 -I heldout/report.py [--model claude-opus-5-5] [--host mac] [--results <jsonl> ...]
"""
import argparse
import json
import statistics
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_RESULTS = [HERE / "runs" / "results-native.jsonl", HERE / "runs" / "results-web.jsonl"]
ARM_ORDER = ["ad-14", "ad-up", "peekaboo", "cua", "bh", "pw", "ab"]
METRICS = [("cli", "CLI s", True), ("wall", "wall s", True), ("tokens", "tokens", True), ("turns", "turns", True)]


def load(paths, model=None, host=None):
    rows, skipped = [], 0
    for path in paths:
        path = Path(path)
        if not path.exists():
            continue
        for line in path.read_text().splitlines():
            if not line.strip():
                continue
            r = json.loads(line)
            if model and r.get("model") != model or host and r.get("host", "mac") != host:
                continue
            if r.get("valid") is False:
                skipped += 1
                continue
            rows.append(normalize(r))
    return rows, skipped


def normalize(r):
    out = {"suite": r.get("suite"), "arm": r.get("arm"), "task": r.get("task"), "model": r.get("model"),
           "ok": bool(r.get("success")), "why": r.get("verify", "")}
    out["cli"] = float(r.get("cli_s") or 0)
    out["wall"] = float(r.get("wall_s") or 0)
    out["tokens"] = int(r.get("input_total") or 0) + int(r.get("output_tokens") or 0)
    out["turns"] = int(r.get("num_turns") or 0)
    return out


def stats(values):
    if not values:
        return None
    return min(values), statistics.median(values), max(values)


def fmt(value, metric):
    if value is None:
        return "–"
    if metric == "tokens":
        return f"{value / 1000:.0f}k" if value >= 10000 else f"{value:.0f}"
    if metric == "turns":
        return f"{value:g}"
    return f"{value:.1f}"


def triple(s, metric):
    return "–" if s is None else f"{fmt(s[0], metric)} / **{fmt(s[1], metric)}** / {fmt(s[2], metric)}"


def group(rows):
    by = defaultdict(list)
    for r in rows:
        by[(r["suite"], r["task"], r["arm"])].append(r)
    return by


def arms_in(rows):
    seen = {r["arm"] for r in rows}
    return [a for a in ARM_ORDER if a in seen] + sorted(seen - set(ARM_ORDER))


def summary_table(rows, suite):
    by = group(rows)
    tasks = sorted({r["task"] for r in rows if r["suite"] == suite})
    lines = [f"### {suite}: sum of per-task medians", "",
             "| arm | CLI s | wall s | tokens | turns | pass | tasks with runs |", "|---|---|---|---|---|---|---|"]
    for arm in arms_in([r for r in rows if r["suite"] == suite]):
        sums, passes, runs, covered = defaultdict(float), 0, 0, 0
        for t in tasks:
            rs = by.get((suite, t, arm), [])
            if not rs:
                continue
            covered += 1
            for m, _, _ in METRICS:
                sums[m] += statistics.median(r[m] for r in rs)
            passes += sum(r["ok"] for r in rs)
            runs += len(rs)
        lines.append(f"| {arm} | {fmt(sums['cli'], 'cli')} | {fmt(sums['wall'], 'wall')} | {fmt(sums['tokens'], 'tokens')} | "
                     f"{fmt(sums['turns'], 'turns')} | {passes}/{runs} | {covered}/{len(tasks)} |")
    return lines


def task_tables(rows, suite):
    by = group(rows)
    lines = []
    for t in sorted({r["task"] for r in rows if r["suite"] == suite}):
        lines += [f"#### {t}", "", "| arm | CLI s min/med/max | wall s | tokens | turns | pass |", "|---|---|---|---|---|---|"]
        for arm in arms_in([r for r in rows if r["suite"] == suite and r["task"] == t]):
            rs = by[(suite, t, arm)]
            cells = [triple(stats([r[m] for r in rs]), m) for m, _, _ in METRICS]
            lines.append(f"| {arm} | {' | '.join(cells)} | {sum(r['ok'] for r in rs)}/{len(rs)} |")
        fails = [f"{r['arm']}: {r['why'][:120]}" for arm in arms_in(rows) for r in by.get((suite, t, arm), []) if not r["ok"]]
        if fails:
            lines += ["", "Failures: " + "; ".join(fails[:12])]
        lines.append("")
    return lines


def losses(rows, focus="ad-14"):
    """Every (task, metric) where the focus arm's median is worse than another arm's, or its pass rate is lower."""
    if not rows:
        return ["- no valid runs yet"]
    by = group(rows)
    out = []
    for (suite, task) in sorted({(r["suite"], r["task"]) for r in rows}):
        mine = by.get((suite, task, focus))
        if not mine:
            out.append(f"- {suite} {task}: no valid {focus} run")
            continue
        my_rate = sum(r["ok"] for r in mine) / len(mine)
        for arm in arms_in(rows):
            theirs = by.get((suite, task, arm))
            if arm == focus or not theirs:
                continue
            rate = sum(r["ok"] for r in theirs) / len(theirs)
            if rate > my_rate:
                out.append(f"- {suite} {task} pass: {focus} {sum(r['ok'] for r in mine)}/{len(mine)} vs {arm} "
                           f"{sum(r['ok'] for r in theirs)}/{len(theirs)}")
            for m, label, lower_better in METRICS:
                a = statistics.median(r[m] for r in mine)
                b = statistics.median(r[m] for r in theirs)
                if (a > b if lower_better else a < b) and b > 0:
                    out.append(f"- {suite} {task} {label}: {focus} {fmt(a, m)} vs {arm} {fmt(b, m)} ({(a - b) / b:+.0%})")
    return out or [f"- none: {focus} is at least as good as every other arm on every task and metric"]


def render(rows, skipped):
    models = sorted({r["model"] for r in rows})
    lines = ["# Held-out benchmark", "",
             f"{len(rows)} valid runs ({skipped} invalid skipped); models: {', '.join(models) or 'none'}.",
             "Medians and min/max are over every valid run, passes and failures alike. Tokens count input "
             "(uncached, cache read, cache write) plus output.", ""]
    for suite in ("native", "web"):
        if any(r["suite"] == suite for r in rows):
            lines += summary_table(rows, suite) + [""]
    lines += ["## Where agent-desktop 1.4 loses", ""] + losses(rows) + [""]
    for suite in ("native", "web"):
        if any(r["suite"] == suite for r in rows):
            lines += [f"## {suite} per task", ""] + task_tables(rows, suite)
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", nargs="*", default=[str(p) for p in DEFAULT_RESULTS])
    ap.add_argument("--model", default=None)
    ap.add_argument("--host", default=None)
    a = ap.parse_args()
    rows, skipped = load(a.results, a.model, a.host)
    print(render(rows, skipped))


if __name__ == "__main__":
    main()
