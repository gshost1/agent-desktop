#!/bin/bash
# Compares `find --name General|About` in System Settings between two binaries. Prints counts,
# roles, refs present and byte sizes only, never names or values. Quits System Settings after.
set -u
H=$(mktemp -d); export AGENT_DESKTOP_HOME=$H
for bin in "$@"; do
  "$bin" launch "System Settings" >/dev/null 2>&1; sleep 2
  for n in General About; do
    t0=$(python3 -c "import time;print(time.time())"); out=$("$bin" find --app "System Settings" --name "$n" 2>/dev/null); t1=$(python3 -c "import time;print(time.time())")
    printf '%s' "$out" | python3 -c '
import json,sys
raw=sys.stdin.read(); d=json.loads(raw) if raw else {}
m=(d.get("data") or {}).get("matches") or []
print(json.dumps({"bin":sys.argv[1],"name":sys.argv[2],"ok":d.get("ok"),"code":(d.get("error") or {}).get("code"),"matches":len(m),"roles":[x.get("role") for x in m][:8],"with_ref":sum(1 for x in m if x.get("ref_id")),"bytes":len(raw),"keys":sorted((d.get("data") or {}).keys()),"secs":round(float(sys.argv[4])-float(sys.argv[3]),2),"err_keys":sorted((d.get("error") or {}).keys()),"details":(d.get("error") or {}).get("details") if isinstance((d.get("error") or {}).get("details"),dict) and all(isinstance(v,(int,bool)) for v in (d.get("error") or {}).get("details").values()) else None}))' "$(basename $(dirname $(dirname $(dirname $bin))))" "$n" "$t0" "$t1"
  done
  /Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/out/macctl quit "System Settings"; sleep 2
done
rm -rf "$H"
