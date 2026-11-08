"""Headline A/B report: upstream at the fork point against fork main, on the 5 tasks.

Reads bench/runs/results.jsonl (with bench/runs/corrections.jsonl applied) and the
build record bench/out/headline-builds.json written by bench/prepare_headline.sh.
A run counts when its tree is one of the recorded builds and its run id is not older
than the build record. Pass or fail is the driver's independent check, where a stray
file outside the target folder is a fail.

Tokens come from the run's `result_usage`: input (uncached), cache read, cache write
(cache creation) and output, and their total. "Suite" is the sum of the per-task
medians, the figure comparable to one pass of Torben's benchmark.

Reproduce: append the hab1-hab3 and hvar1-hvar3 lines of bench/plan-phase1e.txt to a plan,
run `bash bench/orchestrate.sh <plan>`, then
`python3 bench/headline.py > bench/HEADLINE.md`.
"""

import argparse
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TASKS = ["weather", "settings", "finder", "textedit", "textedit-seeded", "calculator"]
TASK_NAMES = {"weather": "Weather", "settings": "System Settings", "finder": "Finder folder",
              "textedit": "TextEdit Save", "textedit-seeded": "TextEdit Save (seeded, not Aside-comparable)",
              "calculator": "Calculator"}
TOKEN_FIELDS = [("input", "input_tokens"), ("cache read", "cache_read_input_tokens"),
                ("cache write", "cache_creation_input_tokens"), ("output", "output_tokens")]


def load_runs(results, since):
    corrections = {}
    path = results.with_name("corrections.jsonl")
    if path.exists():
        corrections = {c["run_id"]: c for c in map(json.loads, path.read_text().splitlines()) if c}
    runs = []
    for line in results.read_text().splitlines():
        if not line.strip():
            continue
        run = json.loads(line)
        if run.get("run_id", "") < since:
            continue
        if run.get("valid") is False:
            continue
        if run["run_id"] in corrections:
            run["success"] = corrections[run["run_id"]]["success"]
        runs.append(run)
    return runs


def tokens(run):
    usage = run.get("result_usage") or {}
    split = {label: usage.get(key, 0) for label, key in TOKEN_FIELDS}
    split["total"] = sum(split.values())
    return split


def median(values):
    return statistics.median(values) if values else 0


def fmt_k(n):
    if n >= 1e6:
        return f"{n / 1e6:.2f}M"
    return f"{n / 1e3:.0f}K" if n >= 1e3 else f"{n:.0f}"


def build_rows(runs):
    rows = {}
    for task in TASKS:
        rs = [r for r in runs if r["task"] == task]
        if not rs:
            continue
        toks = [tokens(r) for r in rs]
        rows[task] = {
            "runs": len(rs),
            "passed": sum(bool(r["success"]) for r in rs),
            "wall": median([r["wall_s"] for r in rs]),
            "calls": median([r["api_calls"] for r in rs]),
            "tokens": {k: median([t[k] for t in toks]) for k in toks[0]},
            "sum_wall": sum(r["wall_s"] for r in rs),
            "sum_tokens": {k: sum(t[k] for t in toks) for k in toks[0]},
        }
    return rows


def render(builds, runs):
    out = []
    by_build = {}
    for label, info in builds.items():
        tree = Path(info["tree"]).name
        rs = [r for r in runs if Path(r.get("tree") or "").name == tree and r.get("config") == info.get("config", "a")]
        by_build[label] = build_rows(rs)
    out.append("| Build | Runs passed | Suite wall s (sum of medians) | Suite tokens, total | input | cache read | cache write | output | All runs: wall s | All runs: tokens |")
    out.append("|---|---|---|---|---|---|---|---|---|---|")
    out.append("| Aside (Torben's run) | 5/5 | 106 | ~1M | | | | | | |")
    out.append("| agent-desktop before the fixes (Torben's run) | 5/5 | 745 | 5.6M | | | | | | |")
    for label, rows in by_build.items():
        if not rows:
            continue
        passed = sum(r["passed"] for r in rows.values())
        n = sum(r["runs"] for r in rows.values())
        suite = {k: sum(r["tokens"][k] for r in rows.values()) for k in ("total", "input", "cache read", "cache write", "output")}
        out.append(f"| {builds[label]['title']} | {passed}/{n} | {sum(r['wall'] for r in rows.values()):.0f} | "
                   f"{fmt_k(suite['total'])} | {fmt_k(suite['input'])} | {fmt_k(suite['cache read'])} | "
                   f"{fmt_k(suite['cache write'])} | {fmt_k(suite['output'])} | "
                   f"{sum(r['sum_wall'] for r in rows.values()):.0f} | {fmt_k(sum(r['sum_tokens']['total'] for r in rows.values()))} |")
    out.append("")
    out.append("Per task, median of the runs:")
    out.append("")
    out.append("| Task | Build | Passed | Wall s | Model calls | Tokens total | input | cache read | cache write | output |")
    out.append("|---|---|---|---|---|---|---|---|---|---|")
    for task in TASKS:
        for label, rows in by_build.items():
            r = rows.get(task)
            if not r:
                continue
            t = r["tokens"]
            out.append(f"| {TASK_NAMES[task]} | {builds[label]['title']} | {r['passed']}/{r['runs']} | {r['wall']:.1f} | "
                       f"{r['calls']:.0f} | {fmt_k(t['total'])} | {fmt_k(t['input'])} | {fmt_k(t['cache read'])} | "
                       f"{fmt_k(t['cache write'])} | {fmt_k(t['output'])} |")
    out.append("")
    out.append("Runs:")
    out.append("")
    out.append("| Run | Build | Task | Passed | Wall s | Calls | Tokens total | Check |")
    out.append("|---|---|---|---|---|---|---|---|")
    for run in sorted(runs, key=lambda r: r["run_id"]):
        label = next((l for l, i in builds.items() if Path(i["tree"]).name == Path(run.get("tree") or "").name
                      and run.get("config") == i.get("config", "a")), None)
        if not label:
            continue
        out.append(f"| {run['run_id']} | {builds[label]['title']} | {TASK_NAMES.get(run['task'], run['task'])} | "
                   f"{'yes' if run['success'] else 'no'} | {run['wall_s']:.1f} | {run['api_calls']} | "
                   f"{fmt_k(tokens(run)['total'])} | {str(run.get('verify', ''))[:70].replace('|', '/')} |")
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--builds", default=str(ROOT / "out" / "headline-builds.json"))
    ap.add_argument("--results", default=str(ROOT / "runs" / "results.jsonl"))
    a = ap.parse_args()
    record = json.loads(Path(a.builds).read_text())
    builds = {
        "upstream": {"tree": record["upstream"]["tree"], "config": "a",
                     "title": f"upstream main at the fork point (`{record['upstream']['head']}`)"},
        "fork": {"tree": record["fork"]["tree"], "config": "a",
                 "title": f"fork main with the fixes (`{record['fork']['head']}`)"},
    }
    if "fork_outline" in record:
        builds["fork_outline"] = {"tree": record["fork_outline"]["tree"], "config": "c",
                                  "title": f"fork + `--outline`, batch-first prompt (`{record['fork_outline']['head']}`)"}
    print(render(builds, load_runs(Path(a.results), record.get("since", ""))))


if __name__ == "__main__":
    main()
