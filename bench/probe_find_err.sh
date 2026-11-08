#!/bin/bash
# Cold-launch System Settings, then `find --name General` up to 4 times; print the error code,
# message and suggestion with every quoted string redacted, and data key names. Quits after.
set -u
H=$(mktemp -d); export AGENT_DESKTOP_HOME=$H
bin=$1
for pass in 1 2 3; do
  "$bin" launch "System Settings" 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); print("launch", d.get("ok"), (d.get("data") or {}).get("launch_path"), (d.get("data") or {}).get("renderer"))'
  for i in 1 2; do
    "$bin" find --app "System Settings" --name General 2>/dev/null | python3 -c '
import json,sys,re
d=json.load(sys.stdin); e=d.get("error") or {}
red=lambda s: re.sub(r"\"[^\"]*\"|\x27[^\x27]*\x27|“[^”]*”","<q>",s or "")
print("find", d.get("ok"), e.get("code"), "|", red(e.get("message"))[:300], "|", red(e.get("suggestion"))[:300], "| details", json.dumps(e.get("details")), "| platform", red(str(e.get("platform_detail")))[:200])'
  done
  /Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/out/macctl quit "System Settings"; sleep 2
done
rm -rf "$H"
