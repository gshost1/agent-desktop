"""Paired A/B summary over bench/runs/results.jsonl (the perf-speed-cost harness).
usage: python3 ab_summary.py <results.jsonl> <since run_id> <treeA> <treeB>
Per task and build: n, pass, median/mean wall_s, median input tokens (input+cache write+cache read),
median turns (api_calls). Suite = sum of per-task medians."""
import json, statistics as st, sys
path, since, A, B = sys.argv[1:5]
rows = [json.loads(l) for l in open(path) if l.strip()]
rows = [r for r in rows if r['run_id'] >= since and r.get('valid', True) is not False and r['tree'] in (A, B)]
tasks = ['weather', 'settings', 'calculator']
def tok(r):
    u = r['result_usage']; return u['input_tokens'] + u['cache_creation_input_tokens'] + u['cache_read_input_tokens']
suite = {A: [0, 0, 0, 0, 0, 0], B: [0, 0, 0, 0, 0, 0]}
print(f"{'task':10} {'build':14} n pass wall_med wall_mean tok_med turns_med cli_med model_med")
for t in tasks:
    for tree in (A, B):
        rs = [r for r in rows if r['task'] == t and r['tree'] == tree]
        if not rs: continue
        w = [r['wall_s'] for r in rs]
        vals = (st.median(w), st.mean(w), st.median(tok(r) for r in rs), st.median(r['api_calls'] for r in rs),
                st.median(r['cli_s'] for r in rs), st.median(r['duration_api_ms'] / 1000 for r in rs))
        s = suite[tree]; s[0] += len(rs); s[1] += sum(bool(r['success']) for r in rs)
        for i in range(4): s[2 + i] += vals[i]
        print(f"{t:10} {tree:14} {len(rs)} {sum(bool(r['success']) for r in rs)}/{len(rs)} {vals[0]:7.1f} {vals[1]:8.1f} {vals[2]/1000:6.1f}K {vals[3]:5} {vals[4]:6.1f} {vals[5]:6.1f}")
for tree in (A, B):
    n, p, wm, wmean, tk, tu = suite[tree]
    print(f"SUITE {tree:14} pass {p}/{n} wall_med_sum {wm:.1f} s wall_mean_sum {wmean:.1f} s tok_med_sum {tk/1000:.1f}K turns_med_sum {tu}")
