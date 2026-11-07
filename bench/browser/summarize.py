#!/usr/bin/env python3
"""Summarize results/runs.jsonl into a task x channel table (medians; success count)."""
import json
import statistics
import sys
from collections import defaultdict

rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
groups = defaultdict(list)
for r in rows:
    groups[(r["task"], r["channel"])].append(r)
med = lambda xs: statistics.median(xs) if xs else 0
print("| task | channel | ok | wall s (med) | turns (med) | input tok (med) | output tok (med) | snapshot tok est (med) |")
print("|---|---|---|---|---|---|---|---|")
for (t, c), rs in sorted(groups.items()):
    print(f"| {t} | {c} | {sum(r['ok'] for r in rs)}/{len(rs)} | {med([r['wall_s'] for r in rs]):.1f} | "
          f"{med([r['num_turns'] or 0 for r in rs]):.0f} | {med([r['total_input'] for r in rs]):,.0f} | "
          f"{med([r['output_tokens'] or 0 for r in rs]):,.0f} | {med([r['snapshot_tokens_est'] for r in rs]):,.0f} |")
