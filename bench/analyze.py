#!/usr/bin/env python3
"""Aggregate bench/runs/results.jsonl into the step-4 breakdown tables.

Time per run: wall = CLI (wrapper-logged agent-desktop time) + model
(`duration_api_ms` reported by claude -p) + other (claude start-up, Bash tool
overhead, harness).

Tokens per run are decomposed exactly from per-call input totals:
  sum_k in_k = N * in_0 + sum_i delta_i * (N - 1 - i),  delta_i = in_{i+1} - in_i
in_0 is the fixed prefix (system prompt + tools + skill + task prompt), split
with calibrated sizes. Each delta_i is split into tool-result tokens (result
bytes / calibrated JSON bytes-per-token, capped at delta_i) by command class,
and the remainder, which is the model's own re-read output (tool calls,
thinking, text).

Usage: python3 bench/analyze.py [--results bench/runs/results.jsonl] [--md bench/RESULTS-tables.md]
"""
import argparse
import json
import statistics
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SKILL_TOKENS = 2592
PROMPT_TOKENS = 200
ADDENDUM_TOKENS = {"a": 0, "b": 120, "c": 110, "d": 95, "e": 325}


def bpt():
    cal = ROOT / "out" / "token_calibration.json"
    if cal.exists():
        rows = [r for r in json.loads(cal.read_text())["rows"] if r["file"].endswith(".json")]
        return sum(r["bytes"] for r in rows) / sum(r["tokens"] for r in rows)
    return 2.05


def result_class(tools):
    classes = [t["class"] for t in tools]
    for c in ["snapshot", "batch", "find", "skills", "action", "get", "wait", "launch", "list", "status", "screenshot", "other"]:
        if c in classes:
            return c
    return "other"


def decompose(run, rate):
    steps = run["steps"]
    n = len(steps)
    if not n:
        return {}
    ins = [s["input_total"] for s in steps]
    out = defaultdict(float)
    prefix = ins[0]
    skill = SKILL_TOKENS + ADDENDUM_TOKENS.get(run["config"], 0)
    out["system+tools"] += n * max(prefix - skill - PROMPT_TOKENS, 0)
    out["skill"] += n * skill
    out["task prompt"] += n * PROMPT_TOKENS
    for i in range(n - 1):
        delta = max(ins[i + 1] - ins[i], 0)
        rereads = n - 1 - i
        res_tok = min(sum(t["result_bytes"] for t in steps[i]["tools"]) / rate, delta)
        cls = result_class(steps[i]["tools"])
        if cls in ("snapshot", "batch"):
            key = "tool output: snapshot"
        elif cls == "skills":
            key = "tool output: skill references"
        else:
            key = "tool output: other commands"
        out[key] += res_tok * rereads
        out["model output re-read (history)"] += (delta - res_tok) * rereads
    return out


def med(xs):
    return statistics.median(xs) if xs else 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", default=str(ROOT / "runs" / "results.jsonl"))
    ap.add_argument("--md", default=None)
    a = ap.parse_args()
    rate = bpt()
    runs = [json.loads(l) for l in Path(a.results).read_text().splitlines() if l.strip()]
    corrections_path = Path(a.results).with_name("corrections.jsonl")
    corrections = {}
    if corrections_path.exists():
        corrections = {c["run_id"]: c for c in map(json.loads, corrections_path.read_text().splitlines()) if c}
    for r in runs:
        if r["run_id"] in corrections:
            r["success"] = corrections[r["run_id"]]["success"]
    lines = [f"Calibrated JSON rate: {rate:.2f} bytes/token. Runs: {len(runs)}.", ""]

    groups = defaultdict(list)
    for r in runs:
        groups[(r["model"], r["config"], r["task"])].append(r)

    lines += ["### Per task and config (median of runs)", "",
              "| model | config | task | runs | success | wall s | model s | CLI s | other s | API calls | input tokens | output tokens |",
              "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for (model, cfg, task), rs in sorted(groups.items()):
        wall = [r["wall_s"] for r in rs]
        api = [(r["duration_api_ms"] or 0) / 1000 for r in rs]
        cli = [r["cli_s"] for r in rs]
        other = [r["wall_s"] - (r["duration_api_ms"] or 0) / 1000 - r["cli_s"] for r in rs]
        lines.append(f"| {model.replace('claude-', '')} | {cfg} | {task} | {len(rs)} | {sum(r['success'] for r in rs)}/{len(rs)} | "
                     f"{med(wall):.1f} | {med(api):.1f} | {med(cli):.1f} | {med(other):.1f} | {med([r['api_calls'] for r in rs]):.0f} | "
                     f"{med([r['input_total'] for r in rs]):,.0f} | {med([r['result_usage'].get('output_tokens', 0) for r in rs]):,.0f} |")

    lines += ["", "### Five-task suite totals per config (sum over tasks of the per-task median)", "",
              "| model | config | success | wall s | model s | CLI s | other s | API calls | input tokens | of which cache read | output tokens |",
              "|---|---|---|---|---|---|---|---|---|---|---|"]
    suites = defaultdict(lambda: defaultdict(float))
    succ = defaultdict(lambda: [0, 0])
    for (model, cfg, task), rs in groups.items():
        s = suites[(model, cfg)]
        s["wall"] += med([r["wall_s"] for r in rs])
        s["api"] += med([(r["duration_api_ms"] or 0) / 1000 for r in rs])
        s["cli"] += med([r["cli_s"] for r in rs])
        s["other"] += med([r["wall_s"] - (r["duration_api_ms"] or 0) / 1000 - r["cli_s"] for r in rs])
        s["calls"] += med([r["api_calls"] for r in rs])
        s["in"] += med([r["input_total"] for r in rs])
        s["cache"] += med([r["cache_read"] for r in rs])
        s["out"] += med([r["result_usage"].get("output_tokens", 0) for r in rs])
        s["tasks"] += 1
        succ[(model, cfg)][0] += sum(r["success"] for r in rs)
        succ[(model, cfg)][1] += len(rs)
    for (model, cfg), s in sorted(suites.items()):
        ok, tot = succ[(model, cfg)]
        lines.append(f"| {model.replace('claude-', '')} | {cfg} | {ok}/{tot} | {s['wall']:.0f} | {s['api']:.0f} | {s['cli']:.0f} | {s['other']:.0f} | "
                     f"{s['calls']:.0f} | {s['in']:,.0f} | {s['cache']:,.0f} | {s['out']:,.0f} |")

    lines += ["", "### Input-token decomposition per config (sum over all runs, share of total)", ""]
    keys = ["system+tools", "skill", "task prompt", "tool output: snapshot", "tool output: skill references",
            "tool output: other commands", "model output re-read (history)"]
    lines += ["| model | config | total input | " + " | ".join(keys) + " |", "|---|---|---|" + "---|" * len(keys)]
    agg = defaultdict(lambda: defaultdict(float))
    for r in runs:
        for k, v in decompose(r, rate).items():
            agg[(r["model"], r["config"])][k] += v
        agg[(r["model"], r["config"])]["_total"] += r["input_total"]
    for (model, cfg), d in sorted(agg.items()):
        tot = d["_total"] or 1
        cells = [f"{d[k] / tot * 100:.0f}%" for k in keys]
        lines.append(f"| {model.replace('claude-', '')} | {cfg} | {d['_total']:,.0f} | " + " | ".join(cells) + " |")

    lines += ["", "### CLI time by command class (all runs)", "", "| class | calls | total s | median s | max s |", "|---|---|---|---|---|"]
    by = defaultdict(list)
    for r in runs:
        for c in r["cli"]:
            first = c["args"].split(" ")[0].lstrip("-") or "?"
            if first == "headed":
                first = c["args"].split(" ")[1]
            by[first].append(c["s"])
    for k, v in sorted(by.items(), key=lambda kv: -sum(kv[1])):
        lines.append(f"| {k} | {len(v)} | {sum(v):.1f} | {med(v):.2f} | {max(v):.2f} |")

    text = "\n".join(lines)
    print(text)
    if a.md:
        Path(a.md).write_text(text + "\n")


if __name__ == "__main__":
    main()
