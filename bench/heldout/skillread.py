#!/usr/bin/env python3
"""What each native run saw of the skill, and what it cost.

Per arm and task: how many runs read the skill in full or got Claude Code's spill preview (a Bash result
over ~30 KB is saved to a tool-results file and the model sees a 2 KB preview), whether the run then read
the spilled file, the skill's token cost (context growth from the skill-read call to the next one), and the
median tokens of the whole run.

Usage: python3 -I heldout/skillread.py --arms ad-up,ad-148b --tasks hn3,hn4,hn5 --results <jsonl>...
"""
import argparse
import json
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import pricing  # noqa: E402

PREVIEW_MAX = 3000


def skill_step(row):
    for k, s in enumerate(row.get("steps") or []):
        for t in s.get("tools") or []:
            if t.get("class") == "skills" or "skills get" in t.get("cmd", ""):
                return k, t
    return None, None


def read_spill(row):
    return any("tool-results/" in t.get("cmd", "") for s in row.get("steps") or [] for t in s.get("tools") or [])


def run_facts(row):
    k, tool = skill_step(row)
    steps = row.get("steps") or []
    facts = {"tokens": int(row.get("input_total") or 0) + pricing.row_output_tokens(row), "skill": tool is not None}
    if tool is None:
        return facts
    facts["bytes"] = int(tool.get("result_bytes") or 0)
    facts["spilled"] = facts["bytes"] < PREVIEW_MAX
    facts["read_spill"] = read_spill(row)
    if k + 1 < len(steps):
        facts["skill_tokens"] = int(steps[k + 1]["input_total"]) - int(steps[k]["input_total"])
    return facts


def table(rows, arms, tasks):
    out = ["| arm | task | runs | full skill | spill preview | read the spill file | skill bytes seen | skill tokens | "
           "run tokens median |", "|---|---|---|---|---|---|---|---|---|"]
    for arm in arms:
        for task in tasks:
            facts = [run_facts(r) for r in rows if r.get("arm") == arm and r.get("task") == task]
            if not facts:
                continue
            read = [f for f in facts if f["skill"]]
            seen = sorted({f["bytes"] for f in read})
            cost = [f["skill_tokens"] for f in read if "skill_tokens" in f]
            out.append(f"| {arm} | {task} | {len(facts)} | {sum(not f['spilled'] for f in read)} | "
                       f"{sum(f['spilled'] for f in read)} | {sum(f['read_spill'] for f in read)} | "
                       f"{', '.join(map(str, seen)) or '-'} | {round(statistics.median(cost) / 1000, 1) if cost else '-'}k | "
                       f"{round(statistics.median(f['tokens'] for f in facts) / 1000, 1)}k |")
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arms", required=True)
    ap.add_argument("--tasks", required=True)
    ap.add_argument("--model", default="claude-sonnet-5-5")
    ap.add_argument("--results", nargs="+", required=True)
    a = ap.parse_args()
    rows = [json.loads(line) for p in a.results for line in Path(p).read_text().splitlines() if line.strip()]
    rows = [r for r in rows if r.get("suite") == "native" and r.get("model") == a.model and r.get("valid") is not False]
    print("\n".join(table(rows, a.arms.split(","), a.tasks.split(","))))


if __name__ == "__main__":
    main()
