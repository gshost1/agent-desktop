#!/bin/bash
# Size of `snapshot --app Calculator -i --compact` per binary, and where the digit buttons fall.
set -u
H=$(mktemp -d); export AGENT_DESKTOP_HOME=$H
for bin in "$@"; do
  "$bin" launch Calculator >/dev/null 2>&1; sleep 1
  out=$("$bin" snapshot --app Calculator -i --compact 2>/dev/null)
  printf '%s' "$out" | python3 -c '
import json,sys
raw=sys.stdin.read(); d=json.loads(raw)
def w(n):
    yield n
    for c in n.get("children",[]) or []: yield from w(c)
nodes=list(w(d["data"]["tree"]))
pos={}
for n in nodes:
    if n.get("role")=="button" and n.get("name") in ("5","7","Five","Seven","Multiply","Equals","All Clear"):
        pos[n.get("name")]=(n.get("ref_id","")[-4:], raw.find(n.get("ref_id","~")))
print(sys.argv[1], "bytes", len(raw), "nodes", len(nodes), "refs", d["data"].get("ref_count"), "button_pos", pos)
print("  sample node keys:", sorted(next(n for n in nodes if n.get("role")=="button").keys()))' "$(basename $(dirname $(dirname $(dirname $bin))))"
  /Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/out/macctl quit Calculator; sleep 1
done
rm -rf "$H"
