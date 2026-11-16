#!/usr/bin/env python3
"""The final run's report, by the analysis fixed before any final row existed (heldout3 FROZEN.md, "Analysis").

Inputs are the saved schedule (every planned cell), the attempt ledger and the result rows. Nothing is dropped:
  - every scheduled cell is one attempt. A cell with a row counts its verdict; a cell whose attempt started and
    never ended (interrupted), ended in a harness error or without a row, or never started counts as a failure in the primary,
    intention-to-test (ITT) analysis. Rows of another experiment, rows outside the schedule and repeated run ids
    are counted and ignored.
  - a sensitivity view leaves out disputed cells (no model call, an infrastructure fault, a crash dialog, a
    harness error, an interruption) for every arm of that task and round, so pairs stay matched.
  - no winner is named unless every cell has an ended attempt with a row. Otherwise the headline is "incomplete
    exploratory comparison".

Primary endpoint: completion, paired by (task, round) against the focus arm. Exact McNemar (sign test over
discordant pairs), Holm-adjusted over the comparators, alpha 0.05. Effect: the difference in completion with a
95% interval from a task-clustered bootstrap (tasks resampled with their rounds, 10000 draws, fixed seed).
Noninferiority is declared only when that interval's lower bound is above -10 points. A p above alpha is reported
as "no difference established", never as a tie.

Efficiency, reported, not ranked on:
  - conditional: on tasks both arms passed at least once, the geometric mean ratio of per-task pass medians
    (focus / comparator) with a task-bootstrap interval;
  - failure-inclusive, raw: mean per attempt over every attempt, and pooled total / successes (undefined with no
    success);
  - the penalized task-cost sum (each task's total / max(passes, 0.5): a task with no pass charged twice its
    total), named as a penalty and shown beside two sensitivities: a zero-pass task charged its total once, and
    charged one cap (300 s) for wall time.

python3 -I final_report.py --schedule S --ledger L --rows R --focus ad-15 [--versions versions.json]
"""
import argparse
import json
import math
import random
import statistics
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import pricing  # noqa: E402
import stats  # noqa: E402

ALPHA = 0.05
NONINFERIORITY = -0.10
BOOT = 10000
BOOT_SEED = 15
CAP_S = 300
METRICS = [("wall", "wall s"), ("tokens", "tokens"), ("cli", "CLI s"), ("turns", "turns")]
DISPUTED = ("interrupted", "harness_error", "no_row", "not_started", "infra_fault", "crash")


def jsonl(path):
    out = []
    if path and Path(path).exists():
        for line in Path(path).read_text().splitlines():
            try:
                out.append(json.loads(line))
            except ValueError:
                continue
    return out


def metrics(row):
    return {"wall": float(row.get("wall_s") or 0), "cli": float(row.get("cli_s") or 0),
            "tokens": int(row.get("input_total") or 0) + pricing.row_output_tokens(row),
            "turns": pricing.row_turns(row), "cost": pricing.row_cost(row)[0]}


def cells(plan, ledger, rows):
    """One record per scheduled cell: arm, task, round, ok, state, metrics; plus counts of ignored rows."""
    exp = plan["experiment"]
    ends, started = {}, set()
    for rec in ledger:
        if rec.get("experiment") != exp or rec.get("slot") is None:
            continue
        slot = int(rec["slot"])
        if rec.get("state") == "started":
            started.add(slot)
        else:
            ends.setdefault(slot, rec)
    by_slot, seen, ignored = {}, set(), {"other experiment": 0, "outside the schedule": 0, "repeated run id": 0}
    slots = {c["slot"]: c for c in plan["cells"]}
    for r in rows:
        if r.get("experiment") != exp:
            ignored["other experiment"] += 1
            continue
        c = slots.get(r.get("slot"))
        if c is None or (c["arm"], c["task"], c["round"]) != (r.get("arm"), r.get("task"), r.get("round")):
            ignored["outside the schedule"] += 1
            continue
        if r.get("run_id") in seen or r["slot"] in by_slot:
            ignored["repeated run id"] += 1
            continue
        seen.add(r.get("run_id"))
        by_slot[r["slot"]] = r
    out = []
    for c in plan["cells"]:
        row, end = by_slot.get(c["slot"]), ends.get(c["slot"])
        if row is not None and row.get("not_run"):
            state = "infra_fault"
        elif row is not None and "crash-report dialog" in str(row.get("invalid_reason", "")):
            state = "crash"
        elif row is not None and row.get("valid") is False:
            state = "infra_fault"
        elif row is not None:
            state = "finished"
        elif end is not None:
            state = "no_row" if end.get("state") in ("finished", "timed_out") else end.get("state", "harness_error")
        elif c["slot"] in started:
            state = "interrupted"
        else:
            state = "not_started"
        m = metrics(row) if row is not None else {"wall": 0.0, "cli": 0.0, "tokens": 0, "turns": 0,
                                                    "cost": (end or {}).get("cost_usd_list")}
        out.append(dict(c, ok=bool(row and row.get("success")) and state == "finished", state=state,
                        violations=list((row or {}).get("violations") or []), grants=(row or {}).get("grants"),
                        global_input=(row or {}).get("global_input"), **m))
    return out, ignored


def complete(cs):
    return all(c["state"] == "finished" for c in cs)


def pairs(cs, focus, other, exclude=frozenset()):
    a = {(c["task"], c["round"]): c for c in cs if c["arm"] == focus}
    b = {(c["task"], c["round"]): c for c in cs if c["arm"] == other}
    keys = sorted(k for k in a.keys() & b.keys() if k not in exclude)
    return [(k, a[k], b[k]) for k in keys]


def disputed_keys(cs):
    return frozenset((c["task"], c["round"]) for c in cs if c["state"] in DISPUTED)


def bootstrap(by_task, statistic, draws=BOOT, seed=BOOT_SEED):
    """95% percentile interval of statistic(list of task groups) over tasks resampled with replacement."""
    tasks = sorted(by_task)
    if not tasks:
        return None, None
    rng = random.Random(seed)
    values = []
    for _ in range(draws):
        sample = [by_task[rng.choice(tasks)] for _ in tasks]
        v = statistic(sample)
        if v is not None and math.isfinite(v):
            values.append(v)
    if len(values) < draws * 0.9:
        return None, None
    values.sort()
    return values[int(0.025 * len(values))], values[min(len(values) - 1, int(0.975 * len(values)))]


def success_comparison(ps):
    """Discordant counts, exact McNemar p, completion difference and its task-bootstrap interval."""
    b = sum(x["ok"] and not y["ok"] for _, x, y in ps)
    c = sum(y["ok"] and not x["ok"] for _, x, y in ps)
    by_task = defaultdict(list)
    for (task, _), x, y in ps:
        by_task[task].append((x["ok"], y["ok"]))

    def diff(groups):
        flat = [p for g in groups for p in g]
        return (sum(x for x, _ in flat) - sum(y for _, y in flat)) / len(flat) if flat else None

    lo, hi = bootstrap(by_task, diff)
    return {"n": len(ps), "focus_only": b, "other_only": c, "p": stats.sign_test(b, c),
            "diff": diff(list(by_task.values())), "lo": lo, "hi": hi}


def holm(ps):
    """Holm-adjusted p values, in the input order."""
    order = sorted(range(len(ps)), key=lambda i: ps[i])
    adj, running = [0.0] * len(ps), 0.0
    for rank, i in enumerate(order):
        running = max(running, min(1.0, (len(ps) - rank) * ps[i]))
        adj[i] = running
    return adj


def conditional_ratio(ps, m):
    """Geometric mean of per-task pass-median ratios (focus / other) over tasks both passed, with its interval."""
    by_task = defaultdict(lambda: ([], []))
    for (task, _), x, y in ps:
        if x["ok"]:
            by_task[task][0].append(x[m])
        if y["ok"]:
            by_task[task][1].append(y[m])
    logs = {t: math.log(statistics.median(a) / statistics.median(b)) for t, (a, b) in by_task.items()
            if a and b and statistics.median(a) > 0 and statistics.median(b) > 0}

    def gm(groups):
        return math.exp(statistics.mean(groups)) if groups else None

    lo, hi = bootstrap({t: v for t, v in logs.items()}, gm)
    return {"tasks": len(logs), "ratio": gm(list(logs.values())), "lo": lo, "hi": hi}


def efficiency(cs, arm, m):
    mine = [c for c in cs if c["arm"] == arm]
    by_task = defaultdict(list)
    for c in mine:
        by_task[c["task"]].append(c)
    total, wins = sum(c[m] for c in mine), sum(c["ok"] for c in mine)
    penalized = once = capped = 0.0
    for rs in by_task.values():
        t, k = sum(r[m] for r in rs), sum(r["ok"] for r in rs)
        penalized += t / max(k, 0.5)
        once += t / k if k else t
        capped += t / k if k else (CAP_S if m == "wall" else t)
    return {"per_attempt": total / len(mine) if mine else None, "pooled_per_success": total / wins if wins else None,
            "penalized": penalized, "once": once, "capped": capped if m == "wall" else None}


def verdict(cmp, adj_p):
    if cmp["diff"] is None:
        return "inconclusive"
    if adj_p < ALPHA and cmp["diff"] > 0:
        return "wins"
    if adj_p < ALPHA and cmp["diff"] < 0:
        return "loses"
    return "inconclusive"


def pct(x):
    return "–" if x is None else f"{x:+.0%}" if abs(x) < 10 else f"{x:+.1f}"


def fmt(x, m=""):
    if x is None:
        return "undefined"
    if m == "tokens":
        return f"{x / 1000:.0f}k"
    return f"{x:.1f}"


def headline(cs, focus, arms, comps, versions, ok_all):
    v = versions or {}
    rate = {a: f"{sum(c['ok'] for c in cs if c['arm'] == a)}/{sum(c['arm'] == a for c in cs)}" for a in arms}
    others = [a for a in arms if a != focus]
    scope = (f"We compared the agent-desktop 1.5 candidate [{v.get(focus, focus)}] with "
             + ", ".join(f"{a} [{v.get(a, a)}]" for a in others)
             + f" on ten frozen native macOS tasks across three apps, {v.get('rounds', 'five')} seeded rounds per "
             f"task, using {v.get('model', 'Sonnet 5.5')} on {v.get('host', '[host]')} on {v.get('date', '[date]')}, "
             "with background-only operations and screenshot capture disabled. These are results for this task "
             "suite and configuration."
             + "".join(f" {n}" for n in v.get("scope_notes", [])))
    lines = ["## Headline", "", f"> {scope}", ""]
    if not ok_all:
        return lines + ["> Incomplete exploratory comparison: some planned cells have no finished attempt, so no "
                        "winner is named.", ""]
    for a in others:
        x, kind = comps[a], comps[a]["verdict"]
        cost = (f"Its conditional wall-time ratio against {a} on tasks both completed was "
                f"{fmt(x['wall']['ratio'])}× (95% {fmt(x['wall']['lo'])}–{fmt(x['wall']['hi'])}).")
        if kind == "wins":
            text = (f"The candidate completed {rate[focus]} attempts against {a}'s {rate[a]}. The paired difference "
                    f"was {pct(x['diff'])} (95% {pct(x['lo'])} to {pct(x['hi'])}; Holm p {x['adj_p']:.3g}). The "
                    f"prespecified analysis supports higher completion than {a} on this suite. {cost}")
        elif kind == "loses":
            text = (f"The candidate completed fewer attempts than {a}: {rate[focus]} versus {rate[a]} on this suite "
                    f"(paired difference {pct(x['diff'])}, 95% {pct(x['lo'])} to {pct(x['hi'])}; Holm p "
                    f"{x['adj_p']:.3g}). The paired analysis supports lower completion. {cost} These results do "
                    "not support an overall superiority claim.")
        else:
            second = (f"The candidate met the prespecified 10-point noninferiority margin against {a} on this suite."
                      if x["lo"] is not None and x["lo"] > NONINFERIORITY else
                      "This experiment did not establish a reliability difference; its precision does not establish "
                      "equivalence.")
            text = (f"The candidate completed {rate[focus]} attempts, versus {a}'s {rate[a]} (paired difference "
                    f"{pct(x['diff'])}, 95% {pct(x['lo'])} to {pct(x['hi'])}; Holm p {x['adj_p']:.3g}). {second} "
                    f"{cost} This is a conditional efficiency result.")
        lines += [f"**Against {a}: {kind}.** {text}", ""]
    return lines


def render(plan, ledger, rows, focus, versions=None):
    cs, ignored = cells(plan, ledger, rows)
    arms = [focus] + [a for a in plan["arms"] if a != focus]
    ok_all = complete(cs)
    disputed = disputed_keys(cs)
    comps = {}
    others = [a for a in arms if a != focus]
    for a in others:
        ps = pairs(cs, focus, a)
        comps[a] = success_comparison(ps)
        comps[a]["sens"] = success_comparison(pairs(cs, focus, a, disputed))
        for m, _ in METRICS:
            comps[a][m] = conditional_ratio(ps, m)
    adj = holm([comps[a]["p"] for a in others])
    for a, p in zip(others, adj):
        comps[a]["adj_p"] = p
        comps[a]["verdict"] = verdict(comps[a], p)
    states = defaultdict(lambda: defaultdict(int))
    for c in cs:
        states[c["arm"]][c["state"]] += 1
    lines = ["# Final native comparison", "",
             f"Experiment `{plan['experiment']}`: {len(plan['cells'])} planned cells ({len(plan['arms'])} arms × "
             f"{len(plan['tasks'])} tasks × {plan['rounds']} rounds, schedule seed {plan['seed']}). "
             f"Ignored rows: {', '.join(f'{k} {v}' for k, v in ignored.items())}.", ""]
    lines += headline(cs, focus, arms, comps, versions, ok_all)
    lines += ["## Attempts", "", "| arm | " + " | ".join(("finished",) + DISPUTED) + " | passes |",
              "|---" * (len(DISPUTED) + 3) + "|"]
    for a in arms:
        lines.append(f"| {a} | " + " | ".join(str(states[a][s]) for s in ("finished",) + DISPUTED) +
                     f" | {sum(c['ok'] for c in cs if c['arm'] == a)} |")
    lines += ["", f"## Completion, paired by task and round against {focus} (primary, ITT)", "",
              f"Exact McNemar over discordant pairs, Holm-adjusted over {len(others)} comparators, alpha {ALPHA}. "
              "Difference = focus minus comparator completion, 95% task-bootstrap interval. A p above alpha means "
              "no difference established, not a tie. Sensitivity: disputed task-rounds left out for every arm.", "",
              "| comparator | pairs | only focus passed | only comparator passed | difference (95%) | McNemar p | "
              "Holm p | verdict | sensitivity: pairs, difference, p |", "|---" * 9 + "|"]
    for a in others:
        x, s = comps[a], comps[a]["sens"]
        lines.append(f"| {a} | {x['n']} | {x['focus_only']} | {x['other_only']} | {pct(x['diff'])} "
                     f"({pct(x['lo'])} to {pct(x['hi'])}) | {x['p']:.3g} | {x['adj_p']:.3g} | {x['verdict']} | "
                     f"{s['n']}, {pct(s['diff'])}, {s['p']:.3g} |")
    lines += ["", "## Efficiency, conditional on success", "",
              "Geometric mean over tasks both arms passed of the focus / comparator pass-median ratio, 95% "
              "task-bootstrap interval. Below 1 favours the focus arm. Wall time is agent-loop time; tool daemon "
              "start before the model (cua) is outside it.", "",
              "| comparator | " + " | ".join(f"{label}: tasks, ratio (95%)" for _, label in METRICS) + " |",
              "|---" * (len(METRICS) + 1) + "|"]
    for a in others:
        lines.append(f"| {a} | " + " | ".join(
            f"{comps[a][m]['tasks']}, {fmt(comps[a][m]['ratio'])} ({fmt(comps[a][m]['lo'])}–{fmt(comps[a][m]['hi'])})"
            for m, _ in METRICS) + " |")
    lines += ["", "## Efficiency including failures", "",
              "Raw: mean per attempt over every attempt; pooled per success is the total over every attempt divided "
              "by all successes (undefined with none). The penalized task-cost sum charges a task with no pass "
              "twice its total; it is a policy penalty, not a measured cost per success, and is shown beside two "
              "conventions: a no-pass task charged its total once, and (wall time) one 300 s cap.", "",
              "| arm | metric | per attempt | pooled per success | penalized task sum (×2) | no-pass once | "
              "no-pass one cap |", "|---" * 7 + "|"]
    for a in arms:
        for m, label in METRICS:
            e = efficiency(cs, a, m)
            lines.append(f"| {a} | {label} | {fmt(e['per_attempt'], m)} | {fmt(e['pooled_per_success'], m)} | "
                         f"{fmt(e['penalized'], m)} | {fmt(e['once'], m)} | "
                         f"{fmt(e['capped'], m) if e['capped'] is not None else '–'} |")
    lines += ["", "## Conditions per arm", "",
              "| arm | runs with an audit hit | foreground (measured or named) | capture | grants seen | runs with "
              "global input |", "|---|---|---|---|---|---|"]
    for a in arms:
        mine = [c for c in cs if c["arm"] == a]
        grants = sorted({json.dumps(c["grants"], sort_keys=True) for c in mine if c["grants"]})
        lines.append(f"| {a} | {sum(bool(c['violations']) for c in mine)} | "
                     f"{sum(any(v.startswith('foreground') for v in c['violations']) for c in mine)} | "
                     f"{sum(any(v.startswith('capture') for v in c['violations']) for c in mine)} | "
                     f"{'; '.join(grants) or 'none recorded'} | {sum(bool(c['global_input']) for c in mine)} |")
    lines += ["", "## Per task", "", "| task | " + " | ".join(arms) + " |", "|---" * (len(arms) + 1) + "|"]
    for t in plan["tasks"]:
        row = []
        for a in arms:
            rs = [c for c in cs if c["arm"] == a and c["task"] == t]
            row.append(f"{sum(c['ok'] for c in rs)}/{len(rs)}")
        lines.append(f"| {t} | " + " | ".join(row) + " |")
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--schedule", required=True)
    ap.add_argument("--ledger", required=True)
    ap.add_argument("--rows", required=True)
    ap.add_argument("--focus", required=True)
    ap.add_argument("--versions", default=None, help="JSON {arm: version, model, host, date, rounds, scope_notes: [sentence, ...]}")
    a = ap.parse_args()
    plan = json.loads(Path(a.schedule).read_text())
    versions = json.loads(Path(a.versions).read_text()) if a.versions else None
    print(render(plan, jsonl(a.ledger), jsonl(a.rows), a.focus, versions), end="")


if __name__ == "__main__":
    main()
