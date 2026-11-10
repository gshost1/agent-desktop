"""usage: launch_ab.py <binA> <binB> <runs> <app>...  alternating cold launches; prints min/median/max of launch s and launch - CG window s"""
import subprocess, sys, time, statistics as st, tempfile, os, re
A, B, N, apps = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4:]
MC = '/Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/out/macctl'
WW = os.path.join(os.path.dirname(__file__), 'winwatch')
BID = {'Weather': 'com.apple.weather', 'Calculator': 'com.apple.calculator', 'System Settings': 'com.apple.systempreferences', 'Font Book': 'com.apple.FontBook'}
home = tempfile.mkdtemp(dir=os.path.dirname(os.path.dirname(__file__))); os.chmod(home, 0o700)
env = dict(os.environ, AGENT_DESKTOP_HOME=home)
res = {}
for app in apps:
    for i in range(N + 1):
        for tag, b in (('A', A), ('B', B)) if i % 2 == 0 else (('B', B), ('A', A)):
            subprocess.run([MC, 'quit', app], capture_output=True); time.sleep(2)
            w = subprocess.Popen([WW, BID[app], '8'], stdout=subprocess.PIPE, text=True)
            t = time.time(); out = subprocess.run([b, 'launch', app], capture_output=True, text=True, env=env).stdout; dt = time.time() - t
            ok = subprocess.run([b, 'snapshot', '--app', app, '-i'], capture_output=True, text=True, env=env).stdout.find('"ok":true') >= 0
            m = re.search(r'cg_window=([\d.]+)', w.communicate()[0])
            if i == 0: continue
            res.setdefault((app, tag), []).append((dt, dt - float(m.group(1)) if m else None, ok))
    subprocess.run([MC, 'quit', app], capture_output=True)
for (app, tag), v in sorted(res.items()):
    l = [x[0] for x in v]; o = [x[1] for x in v if x[1] is not None]
    f = lambda xs: 'min %.2f med %.2f max %.2f' % (min(xs), st.median(xs), max(xs)) if xs else '-'
    print(f"{app:16} {tag} n={len(v)} snapshot_ok={sum(x[2] for x in v)} launch {f(l)} | after_cg_window {f(o)}")
