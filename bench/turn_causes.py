#!/usr/bin/env python3
"""Label each tool turn beyond the first look and the final read in tonight's main runs
(weather, settings, calculator) by its cause. Heuristic; see TASKS.md for the hand-checked counts."""
import json, re, collections, statistics
UNBRACED = re.compile(r'\$S:e')
def turns_of(d):
    by = collections.OrderedDict()
    for s in d['steps']:
        if s['tools']: by.setdefault(s['call'], []).extend(s['tools'])
    return list(by.values())
def kind(tools):
    c = ' ; '.join(t['cmd'] for t in tools)
    return c
def keys(c):
    m = re.search(r'for e in ([^;]+);', c); return m.group(1).replace('e','').split() if m else None
def label(task, prev, cur):
    c = kind(cur); p = kind(prev) if prev else ''
    if '--help' in c: return 'help_lookup'
    if task == 'calculator':
        if UNBRACED.search(p) and ('jq -c .error' in c or not UNBRACED.search(c)): return 'zsh_unbraced_ref'
        if keys(c) and keys(p) and keys(c) != keys(p): return 'wrong_key_retry'
        if keys(c) and keys(p): return 'same_keys_rerun'
        return 'result_reread'
    if 'find ' in c and 'snapshot' not in c: return 'find_retry_or_locate'
    if 'click' in c: return 'click_turn_after_extra_locate' if task=='settings' else 'click'
    if '--root' in c and '--root' in p: return 'redrill_same_root'
    if '--root' in c: return 'drill'
    prev_trunc = any('snapshot' in t['cmd'] and 'head -c' in t['cmd'] and '--root' not in t['cmd'] and t['result_bytes'] >= 3900 for t in (prev or []))
    if prev_trunc: return 'reread_after_head_c_truncation'
    if 'snapshot' in p and '--root' not in p: return 'refilter_same_tree'
    if 'click' in p: return 'post_click_reread'
    return 'other'
runs = [json.loads(l) for l in open('runs/results.jsonl')]
runs = [d for d in runs if d['run_id'].startswith('1108') and d['tree'] in ('wt-final','wt-main67') and d['task'] in ('weather','settings','calculator')]
out = collections.defaultdict(collections.Counter); stat = {}
for task in ('weather','settings','calculator'):
    rs = [d for d in runs if d['task']==task]
    extra_total = 0
    for d in rs:
        T = turns_of(d)
        lab = []
        start = 0
        if T and all(t['class']=='launch' for t in T[0]):
            lab.append('launch_alone_turn'); start = 1
        for i in range(start+1, len(T)-1):
            lab.append(label(task, T[i-1], T[i]))
        if d['num_turns'] - 1 > len(T): lab += ['no_tool_turn'] * (d['num_turns']-1-len(T))
        extra_total += len(lab)
        out[task].update(lab)
    stat[task] = dict(runs=len(rs), turns_median=statistics.median(d['num_turns'] for d in rs),
        turns_min=min(d['num_turns'] for d in rs), turns_max=max(d['num_turns'] for d in rs), extra_turns=extra_total,
        success=sum(d['success'] for d in rs))
for task in stat:
    print(task, stat[task]); 
    for k,v in out[task].most_common(): print('   ', v, k)
