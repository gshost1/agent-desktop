#!/usr/bin/env python3
"""Held-out benchmark report from the results jsonl files, as Markdown on stdout.

Metrics, in this order everywhere:
  CLI s    seconds inside the arm's tool, summed over the run (wrapper log, arms/logwrap.zsh)
  wall s   whole run, claude -p start to exit
  tokens   input (uncached + cache read + cache write) plus output, summed over the run
  turns    claude -p num_turns

Statistics are kept apart, because a tool that fails fast looks fast in any median that counts failures:
  pass          verifier passes / valid runs
  per success   sum of the metric over every run of the task (failures included) divided by the passes:
                what one success costs. A failure adds its cost and removes a success, so failing never helps.
  pass median   median over passing runs only
  all median    median over every valid run, passes and failures alike (the v1 report's only number)
  p90, max      over every valid run; p90 interpolates linearly between order statistics (n=5: between the
                4th and 5th value)

Ranking: most passes first; ties go to the lower per-success wall time summed over the tasks where every
ranked arm has at least one pass. A ranked arm can never move up by failing a run.

Usage: python3 -I heldout/report.py [--model M] [--host H] [--focus ad-14] [--arms a,b] [--results <jsonl> ...]
"""
import argparse
import json
import math
import statistics
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_RESULTS = [HERE / "runs" / "results-native.jsonl", HERE / "runs" / "results-web.jsonl"]
ARM_ORDER = ["ad-main", "ad-14", "ad-137", "ad-up", "peekaboo", "cua", "bh", "pw", "ab"]
METRICS = [("cli", "CLI s"), ("wall", "wall s"), ("tokens", "tokens"), ("turns", "turns")]


def load(paths, model=None, host=None, arms=None):
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
            if arms and r.get("arm") not in arms:
                continue
            if r.get("valid") is False:
                skipped += 1
                continue
            rows.append(normalize(r))
    return rows, skipped


def normalize(r):
    out = {"suite": r.get("suite"), "arm": r.get("arm"), "task": r.get("task"), "model": r.get("model"),
           "ok": bool(r.get("success")), "why": r.get("verify", ""), "run_id": r.get("run_id"), "round": r.get("round")}
    out["cli"] = float(r.get("cli_s") or 0)
    out["wall"] = float(r.get("wall_s") or 0)
    out["tokens"] = int(r.get("input_total") or 0) + int(r.get("output_tokens") or 0)
    out["turns"] = int(r.get("num_turns") or 0)
    return out


def p90(values):
    v = sorted(values)
    if len(v) == 1:
        return v[0]
    pos = 0.9 * (len(v) - 1)
    lo = math.floor(pos)
    return v[lo] + (v[min(lo + 1, len(v) - 1)] - v[lo]) * (pos - lo)


def cell_stats(rs, m):
    """pass median, all median, p90, max and per-success for one (task, arm) group and metric."""
    values = [r[m] for r in rs]
    passing = [r[m] for r in rs if r["ok"]]
    passes = len(passing)
    return {"pass_med": statistics.median(passing) if passing else None, "all_med": statistics.median(values),
            "p90": p90(values), "max": max(values), "per_success": sum(values) / passes if passes else None}


def fmt(value, metric):
    if value is None:
        return "–"
    if value == math.inf:
        return "∞"
    if metric == "tokens":
        return f"{value / 1000:.0f}k" if value >= 10000 else f"{value:.0f}"
    if metric == "turns":
        return f"{value:.1f}".rstrip("0").rstrip(".")
    return f"{value:.1f}"


def group(rows):
    by = defaultdict(list)
    for r in rows:
        by[(r["suite"], r["task"], r["arm"])].append(r)
    return by


def arms_in(rows):
    seen = {r["arm"] for r in rows}
    return [a for a in ARM_ORDER if a in seen] + sorted(seen - set(ARM_ORDER))


def tasks_in(rows, suite):
    return sorted({r["task"] for r in rows if r["suite"] == suite})


def passes(rs):
    return sum(r["ok"] for r in rs)


def common_tasks(by, suite, tasks, arms):
    """Tasks where every arm has at least one valid run and one pass: sums over them compare like with like."""
    return [t for t in tasks if all(passes(by.get((suite, t, a), [])) > 0 for a in arms)]


def ranking(rows, suite):
    """Arms ordered by passes (desc), then per-success wall summed over the common tasks (asc)."""
    by = group(rows)
    srows = [r for r in rows if r["suite"] == suite]
    tasks, arms = tasks_in(rows, suite), arms_in(srows)
    common = common_tasks(by, suite, tasks, arms)
    out = []
    for arm in arms:
        rs = [r for r in srows if r["arm"] == arm]
        per = {m: sum(cell_stats(by[(suite, t, arm)], m)["per_success"] for t in common) for m, _ in METRICS}
        zero = sum(1 for t in tasks if by.get((suite, t, arm)) and passes(by[(suite, t, arm)]) == 0)
        out.append({"arm": arm, "passes": passes(rs), "runs": len(rs), "per": per, "zero_tasks": zero,
                    "covered": sum(1 for t in tasks if by.get((suite, t, arm)))})
    out.sort(key=lambda x: (-x["passes"] / max(x["runs"], 1), x["per"]["wall"]))
    return out, common, tasks


def summary_tables(rows, suite):
    by = group(rows)
    ranked, common, tasks = ranking(rows, suite)
    n_common = f"{len(common)}/{len(tasks)}"
    lines = [f"### {suite}: ranking (cannot reward failing)", "",
             f"Order: pass rate, then per-success wall time. Per-success sums cover the {n_common} tasks where every arm "
             "passes at least once.", "",
             "| rank | arm | pass | rate | tasks with 0 passes | CLI s / success | wall s / success | tokens / success | "
             "turns / success |", "|---|---|---|---|---|---|---|---|---|"]
    for i, x in enumerate(ranked, 1):
        per = " | ".join(fmt(x["per"][m], m) for m, _ in METRICS)
        lines.append(f"| {i} | {x['arm']} | {x['passes']}/{x['runs']} | {x['passes'] / max(x['runs'], 1):.0%} | "
                     f"{x['zero_tasks']} | {per} |")
    lines += ["", f"### {suite}: sums of per-task statistics over the {n_common} common tasks", "",
              "Each cell: pass median / all median / p90 / max. Pass median counts passing runs only; the other three "
              "count every valid run.", "", "| arm | " + " | ".join(label for _, label in METRICS) + " |",
              "|---|" + "---|" * len(METRICS)]
    for x in ranked:
        cells = []
        for m, _ in METRICS:
            stats = [cell_stats(by[(suite, t, x["arm"])], m) for t in common]
            cells.append(" / ".join(fmt(sum(s[k] for s in stats), m) for k in ("pass_med", "all_med", "p90", "max")))
        lines.append(f"| {x['arm']} | " + " | ".join(cells) + " |")
    lines += ["", f"### {suite}: sum of per-task all-run medians over every task (v1 number; rewards fast failures)", "",
              "| arm | " + " | ".join(label for _, label in METRICS) + " | pass | tasks with runs |",
              "|---|" + "---|" * (len(METRICS) + 2)]
    for x in ranked:
        sums = [fmt(sum(cell_stats(by[(suite, t, x["arm"])], m)["all_med"] for t in tasks if by.get((suite, t, x["arm"]))), m)
                for m, _ in METRICS]
        lines.append(f"| {x['arm']} | " + " | ".join(sums) + f" | {x['passes']}/{x['runs']} | {x['covered']}/{len(tasks)} |")
    return lines


def task_tables(rows, suite):
    by = group(rows)
    lines = []
    for t in tasks_in(rows, suite):
        lines += [f"#### {t}", "", "Cells: pass median / all median / p90 / max; per success after the bar.", "",
                  "| arm | pass | " + " | ".join(label for _, label in METRICS) + " |", "|---|---|" + "---|" * len(METRICS)]
        trows = [r for r in rows if r["suite"] == suite and r["task"] == t]
        for arm in arms_in(trows):
            rs = by[(suite, t, arm)]
            cells = []
            for m, _ in METRICS:
                s = cell_stats(rs, m)
                cells.append(" / ".join(fmt(s[k], m) for k in ("pass_med", "all_med", "p90", "max")) +
                             f" \\| {fmt(s['per_success'], m)}")
            lines.append(f"| {arm} | {passes(rs)}/{len(rs)} | " + " | ".join(cells) + " |")
        fails = [f"{r['arm']}: {r['why'][:120]}" for arm in arms_in(trows) for r in by.get((suite, t, arm), []) if not r["ok"]]
        if fails:
            lines += ["", "Failures: " + "; ".join(fails[:12])]
        lines.append("")
    return lines


def loss_rows(rows, focus):
    """(suite, task, metric, focus value, arm, their value) where another arm beats the focus arm.

    Pass: a higher pass rate. A metric: a lower pass median, counted only when that arm passes at least as
    often as the focus arm, so a faster but less reliable arm is not a loss."""
    by = group(rows)
    out = []
    for suite, task in sorted({(r["suite"], r["task"]) for r in rows}):
        mine = by.get((suite, task, focus))
        if not mine:
            out.append((suite, task, "no run", "", "", ""))
            continue
        my_rate = passes(mine) / len(mine)
        for arm in arms_in(rows):
            theirs = by.get((suite, task, arm))
            if arm == focus or not theirs:
                continue
            rate = passes(theirs) / len(theirs)
            if rate > my_rate:
                out.append((suite, task, "pass", f"{passes(mine)}/{len(mine)}", arm, f"{passes(theirs)}/{len(theirs)}"))
            if rate < my_rate:
                continue
            for m, label in METRICS:
                a, b = cell_stats(mine, m)["pass_med"], cell_stats(theirs, m)["pass_med"]
                if a is None and b is not None:
                    out.append((suite, task, label, "no pass", arm, fmt(b, m)))
                elif a is not None and b is not None and a > b and b > 0:
                    out.append((suite, task, label, fmt(a, m), arm, f"{fmt(b, m)} ({(a - b) / b:+.0%})"))
    return out


def losses(rows, focus="ad-14"):
    lost = loss_rows(rows, focus)
    if not rows:
        return ["- no valid runs yet"]
    if not lost:
        return [f"- none: {focus} is at least as good as every other arm on every task and metric"]
    lines = [f"| suite | task | metric | {focus} | arm | theirs |", "|---|---|---|---|---|---|"]
    return lines + [f"| {' | '.join(str(c) for c in row)} |" for row in lost]


def render(rows, skipped, focus="ad-14"):
    models = sorted({r["model"] for r in rows})
    lines = ["# Held-out benchmark", "",
             f"{len(rows)} valid runs ({skipped} invalid skipped); models: {', '.join(models) or 'none'}.",
             "Tokens count input (uncached, cache read, cache write) plus output. See the module docstring of "
             "report.py for every statistic.", ""]
    for suite in ("native", "web"):
        if any(r["suite"] == suite for r in rows):
            lines += summary_tables(rows, suite) + [""]
    lines += [f"## Where {focus} loses (pass rate, then pass medians against arms at least as reliable)", ""]
    lines += losses(rows, focus) + [""]
    for suite in ("native", "web"):
        if any(r["suite"] == suite for r in rows):
            lines += [f"## {suite} per task", ""] + task_tables(rows, suite)
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", nargs="*", default=[str(p) for p in DEFAULT_RESULTS])
    ap.add_argument("--model", default=None)
    ap.add_argument("--host", default=None)
    ap.add_argument("--focus", default="ad-14")
    ap.add_argument("--arms", default=None, help="comma-separated arms to keep")
    a = ap.parse_args()
    rows, skipped = load(a.results, a.model, a.host, set(a.arms.split(",")) if a.arms else None)
    print(render(rows, skipped, a.focus))


if __name__ == "__main__":
    main()
