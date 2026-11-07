#!/usr/bin/env bash
# A/B for fix/automation-probe-deadline. Times status and a Calculator snapshot
# (min/median/max of 7 after a warm-up) for the fork-main and fix binaries with no
# Automation request pending. The pending-prompt case is covered by unit tests: the
# bench never sends Apple Events, so it cannot raise a consent prompt. Run under
# the bench lock.
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
} 2>&1 | tee bench/out/ab_probe.txt
