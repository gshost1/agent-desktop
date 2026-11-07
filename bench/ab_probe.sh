#!/usr/bin/env bash
# A/B for fix/automation-probe-deadline. Times status and a Calculator snapshot
# (min/median/max of 7 after a warm-up) for the fork-main and fix binaries, first
# with no Automation request pending, then while an osascript request to System
# Events waits on a consent prompt (only reproducible while the "control System
# Events" prompt is unanswered). Run under the bench lock.
set -u
cd "$(dirname "$0")/.."
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-ab"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
open -g -a Calculator; sleep 2
measure() {
    python3 - "$@" <<'PY'
import statistics, subprocess, sys, time
label, binary, *args = sys.argv[1:]
ms = []
for i in range(8):
    t0 = time.perf_counter()
    try:
        subprocess.run([binary, *args], capture_output=True, timeout=200)
    except subprocess.TimeoutExpired:
        pass
    if i:
        ms.append((time.perf_counter() - t0) * 1000)
print(f"{label:34} {' '.join(args):28} min {min(ms):8.0f}  median {statistics.median(ms):8.0f}  max {max(ms):8.0f} ms")
PY
}
{
for side in base:bench/out/wt-base/target/release/agent-desktop head:bench/out/wt-aeprobe/target/release/agent-desktop; do
    measure "idle ${side%%:*}" "${side#*:}" status
    measure "idle ${side%%:*}" "${side#*:}" snapshot --app Calculator -i
done
for side in base:bench/out/wt-base/target/release/agent-desktop head:bench/out/wt-aeprobe/target/release/agent-desktop; do
    for args in "status" "snapshot --app Calculator -i"; do
        osascript -e 'tell application "System Events" to get name of first process' >/dev/null 2>&1 &
        pid=$!; sleep 2
        t0=$(python3 -c 'import time;print(time.time())')
        out=$(timeout 200 "${side#*:}" $args 2>/dev/null | head -c 300)
        t1=$(python3 -c 'import time;print(time.time())')
        printf "pending %-5s %-28s %8.0f ms  %s\n" "${side%%:*}" "$args" "$(echo "($t1-$t0)*1000" | bc)" "$(echo "$out" | grep -o '"automation":{[^}]*}\|"ok":[a-z]*' | tr '\n' ' ')"
        kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
    done
done
} 2>&1 | tee bench/out/ab_probe.txt
