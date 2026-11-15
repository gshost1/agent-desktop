#!/usr/bin/env python3
"""Held-out benchmark report from the results jsonl files, as Markdown on stdout.

Metrics, in this order everywhere:
  CLI s    seconds inside the arm's tool, summed over the run (wrapper log, arms/logwrap.zsh)
  wall s   whole run, claude -p start to exit
  tokens   input (uncached + cache read + cache write) plus output, summed over the run; output comes from the
           final result usage (the per-call stream usage undercounts it), so a capped run has a lower bound
  turns    claude -p num_turns; a run stopped at the cap has none, so its API call count stands in

Statistics are kept apart, because a tool that fails fast looks fast in any median that counts failures:
  pass          verifier passes / valid runs
  per success   sum of the metric over every run of the task (failures included) divided by the passes:
                what one success costs. A failure adds its cost and removes a success, so failing never helps.
                With no pass the divisor is 0.5 (charged), or none in the per-task tables.
  pass median   median over passing runs only
  all median    median over every valid run, passes and failures alike (the v1 report's only number)
  p90, max      over every valid run; p90 interpolates linearly between order statistics (n=5: between the
                4th and 5th value)

Ranking: only arms that ran every task of the suite are ranked; the rest get their own unranked table. Order:
pass rate, then tasks not passed, then per-success wall time summed over every task, where a task with no pass is
charged twice its total (as if half a run had passed). A ranked arm can never move up by failing a run, and no task
leaves the sums because some arm failed it.

Intervals and tests (stats.py): each arm's pass rate carries a Wilson 95% interval. Against the focus arm, Fisher's
exact test compares overall pass rates, and a sign test over the tasks both arms passed compares per-task pass
medians of wall time and tokens. Audit hits and foreground use are counted per arm from each row's violations.

Usage: python3 -I heldout/report.py [--model M] [--host H] [--focus ad-14] [--arms a,b] [--results <jsonl> ...]
"""
import argparse
import json
import math
import statistics
from collections import defaultdict
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import pricing  # noqa: E402
import stats  # noqa: E402

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
    out["tokens"] = int(r.get("input_total") or 0) + pricing.row_output_tokens(r)
    out["turns"] = pricing.row_turns(r)
    out["cost"], out["cost_estimated"] = pricing.row_cost(r)
    out["timed_out"] = bool(r.get("timed_out"))
    out["rules"] = r.get("rules", "v1")
    out["violations"] = list(r.get("violations") or [])
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


NO_PASS_SHARE = 0.5


def charged(rs, m):
    """Per-success cost of one (task, arm) group: the total over its runs divided by its passes.

    A group with no pass is charged as if half a run had passed, twice its total, so a task an arm never
    passes stays in its sums and costs more than the same runs with one pass would."""
    total = sum(r[m] for r in rs)
    return total / max(passes(rs), NO_PASS_SHARE)


def ranking(rows, suite):
    """(ranked, partial, tasks). Ranked arms ran every task of the suite; partial arms did not and are never ranked.

    Ranked order: pass rate (desc), then tasks with no pass (asc), then charged per-success wall summed over
    every task (asc)."""
    by = group(rows)
    srows = [r for r in rows if r["suite"] == suite]
    tasks, arms = tasks_in(rows, suite), arms_in(srows)
    ranked, partial = [], []
    for arm in arms:
        rs = [r for r in srows if r["arm"] == arm]
        ran = [t for t in tasks if by.get((suite, t, arm))]
        per = {m: sum(charged(by[(suite, t, arm)], m) for t in ran) for m, _ in METRICS}
        zero = [t for t in ran if passes(by[(suite, t, arm)]) == 0]
        entry = {"arm": arm, "passes": passes(rs), "runs": len(rs), "per": per, "zero": zero, "ran": ran}
        (ranked if len(ran) == len(tasks) else partial).append(entry)
    ranked.sort(key=lambda x: (-x["passes"] / max(x["runs"], 1), len(x["zero"]), x["per"]["wall"]))
    partial.sort(key=lambda x: x["arm"])
    return ranked, partial, tasks


def summary_tables(rows, suite):
    by = group(rows)
    ranked, partial, tasks = ranking(rows, suite)
    n = len(tasks)
    head = ("| arm | pass | rate | 95% CI | tasks not passed | CLI s / success | wall s / success | tokens / success | "
            "turns / success |")
    lines = [f"### {suite}: ranking (cannot reward failing)", "",
             f"Only arms that ran all {n} tasks are ranked. Order: pass rate, then tasks not passed, then per-success "
             "wall time. Per-success sums cover every task: a task's total over its runs divided by its passes, and "
             "a task with no pass is charged twice its total (as if half a run had passed).", "",
             "| rank " + head, "|---" * 10 + "|"]
    for i, x in enumerate(ranked, 1):
        per = " | ".join(fmt(x["per"][m], m) for m, _ in METRICS)
        zero = ", ".join(x["zero"]) if x["zero"] else "0"
        lines.append(f"| {i} | {x['arm']} | {x['passes']}/{x['runs']} | {x['passes'] / max(x['runs'], 1):.0%} | "
                     f"{interval(x['passes'], x['runs'])} | {zero} | {per} |")
    if partial:
        lines += ["", f"### {suite}: not ranked (ran only some of the {n} tasks)", "",
                  "Sums cover only the tasks the arm ran, so they do not compare with the ranking above.", "",
                  "| arm | tasks run " + head[5:], "|---" * 10 + "|"]
        for x in partial:
            per = " | ".join(fmt(x["per"][m], m) for m, _ in METRICS)
            zero = ", ".join(x["zero"]) if x["zero"] else "0"
            lines.append(f"| {x['arm']} | {len(x['ran'])}/{n}: {','.join(x['ran'])} | {x['passes']}/{x['runs']} | "
                         f"{x['passes'] / max(x['runs'], 1):.0%} | {interval(x['passes'], x['runs'])} | {zero} | "
                         f"{per} |")
    lines += ["", f"### {suite}: sums of per-task statistics over every task an arm ran", "",
              "Each cell: pass median / all median / p90 / max. Pass median counts passing runs only and sums only "
              "the tasks with a pass (marked * when some task has none); the other three count every valid run.", "",
              "| arm | tasks | " + " | ".join(label for _, label in METRICS) + " |",
              "|---|---|" + "---|" * len(METRICS)]
    for x in ranked + partial:
        cells = []
        for m, _ in METRICS:
            stats = [cell_stats(by[(suite, t, x["arm"])], m) for t in x["ran"]]
            pm = [s["pass_med"] for s in stats if s["pass_med"] is not None]
            first = fmt(sum(pm), m) + ("*" if len(pm) < len(stats) else "")
            cells.append(" / ".join([first] + [fmt(sum(s[k] for s in stats), m) for k in ("all_med", "p90", "max")]))
        lines.append(f"| {x['arm']} | {len(x['ran'])}/{n} | " + " | ".join(cells) + " |")
    return lines


def interval(k, n):
    lo, hi = stats.wilson(k, n)
    return f"{lo:.0%}–{hi:.0%}"


FOREGROUND_PREFIX = "foreground"


def paired_rows(rows, suite, focus):
    """One row per other arm: pass rates with Fisher p, and per-task sign tests on wall and tokens pass medians."""
    by = group(rows)
    srows = [r for r in rows if r["suite"] == suite]
    mine = [r for r in srows if r["arm"] == focus]
    out = []
    for arm in arms_in(srows):
        theirs = [r for r in srows if r["arm"] == arm]
        if arm == focus or not mine:
            continue
        row = {"arm": arm, "focus": f"{passes(mine)}/{len(mine)}", "theirs": f"{passes(theirs)}/{len(theirs)}",
               "fisher": stats.fisher(passes(mine), len(mine), passes(theirs), len(theirs))}
        for m in ("wall", "tokens"):
            wins = losses_ = ties = 0
            for t in tasks_in(rows, suite):
                a, b = by.get((suite, t, focus)), by.get((suite, t, arm))
                if not a or not b:
                    continue
                ma, mb = cell_stats(a, m)["pass_med"], cell_stats(b, m)["pass_med"]
                if ma is None or mb is None:
                    continue
                wins, losses_, ties = wins + (ma < mb), losses_ + (ma > mb), ties + (ma == mb)
            row[m] = (wins, losses_, ties, stats.sign_test(wins, losses_))
        out.append(row)
    return out


def paired_tables(rows, suite, focus):
    lines = [f"### {suite}: paired against {focus}", "",
             f"Fisher's exact p compares overall pass rates. Wall and tokens: tasks where {focus}'s pass median is "
             "lower / higher / tied, over tasks both arms passed, with the two-sided sign-test p. A p above 0.05 is "
             "inside the noise.", "",
             f"| arm | {focus} pass | arm pass | Fisher p | wall {focus} lower/higher/tie | p | "
             f"tokens {focus} lower/higher/tie | p |", "|---" * 8 + "|"]
    for x in paired_rows(rows, suite, focus):
        w, t = x["wall"], x["tokens"]
        lines.append(f"| {x['arm']} | {x['focus']} | {x['theirs']} | {x['fisher']:.3g} | {w[0]}/{w[1]}/{w[2]} | "
                     f"{w[3]:.3g} | {t[0]}/{t[1]}/{t[2]} | {t[3]:.3g} |")
    return lines


def rule_tables(rows, suite):
    srows = [r for r in rows if r["suite"] == suite]
    lines = [f"### {suite}: rules and audit", "",
             "Runs per audit rule set, runs with an audit hit, and runs that used the tool's foreground mode.", "",
             "| arm | rules | runs with an audit hit | of them foreground |", "|---|---|---|---|"]
    for arm in arms_in(srows):
        rs = [r for r in srows if r["arm"] == arm]
        sets = ", ".join(f"{k} {sum(r['rules'] == k for r in rs)}" for k in sorted({r["rules"] for r in rs}))
        hit = sum(bool(r["violations"]) for r in rs)
        fg = sum(any(v.startswith(FOREGROUND_PREFIX) for v in r["violations"]) for r in rs)
        lines.append(f"| {arm} | {sets} | {hit} | {fg} |")
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
            lines += paired_tables(rows, suite, focus) + [""] + rule_tables(rows, suite) + [""]
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
