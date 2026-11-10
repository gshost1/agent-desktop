"""usage: mkplan.py <label prefix> <treeA> <treeB> <prelude.sh>  -> plan on stdout, rotation copied from plan-final-ab.txt"""
import sys
pre, A, B, prelude = sys.argv[1:5]
src = open('/Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/plan-final-ab.txt').read().splitlines()
print(f"# {pre}: A={A} vs B={B}, weather/settings/calculator, n=8 per task per side, Opus 5.5 cfg a, rotation of plan-final-ab.txt, desktop-Space prelude before each entry")
for line in src:
    if line.startswith('#') or not line.strip(): continue
    label, *entries = line.split()
    out = []
    for e in entries:
        e = e.replace('bench/out/wt-upstream', A).replace('bench/out/wt-final', B)
        out += [f"sh:{prelude}", e]
    print(f"{pre}{label[2:]} " + " ".join(out))
