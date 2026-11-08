#!/usr/bin/env bash
# Live check for fix/window-title-from-ax (Screen Recording off on this Mac):
#   1. list-windows for the E2E fixture, base (fork main) against head: does the
#      title read "AgentDesk Fixture" (the AX title) instead of the app name?
#   2. the macOS E2E on head with the harness-prerequisite fix (fix/e2e-prerequisites)
#      merged in a throwaway checkout, so the run goes past the fixture gate
#   3. perf-baseline-compare of head against fork main
cd "$(dirname "$0")/.."
root="$PWD"
out=bench/out/check_window_title.txt; : > "$out"
export AGENT_DESKTOP_HOME="$root/bench/out/home-probe"
git -C bench/out/wt-wintitle fetch -q origin && git -C bench/out/wt-wintitle checkout -q --detach origin/fix/window-title-from-ax
echo "head $(git -C bench/out/wt-wintitle rev-parse --short HEAD)" | tee -a "$out"
source bench/cargo_env.sh
bench_build bench/out/wt-wintitle
fixture=bench/out/perf-wt-cold/fixture/AgentDeskFixture.app/Contents/MacOS/AgentDeskFixture
AGENT_DESKTOP_FIXTURE_NO_ACTIVATE=1 "$fixture" >/dev/null 2>&1 &
fpid=$!
sleep 2
for side in main wintitle; do
    echo "$side $(bench/out/wt-$side/target/release/agent-desktop list-windows --app AgentDeskFixture | python3 -c 'import json,sys; print([(w["title"], w["pid"], w["visible"]) for w in json.load(sys.stdin)["data"]])')" | tee -a "$out"
    echo "$side all-apps $(bench/out/wt-$side/target/release/agent-desktop list-windows | python3 -c 'import json,sys; d=json.load(sys.stdin)["data"]; print(len(d), sum(w["title"]==w["app_name"] for w in d), "titles equal to the app name")')" | tee -a "$out"
done
kill "$fpid" 2>/dev/null; wait "$fpid" 2>/dev/null
e2e=bench/out/wt-wintitle-e2e
[ -d "$e2e" ] || git worktree add -q --detach "$e2e" origin/fix/window-title-from-ax
git -C "$e2e" fetch -q origin
git -C "$e2e" checkout -q --detach origin/fix/window-title-from-ax
git -C "$e2e" -c user.name=bench -c user.email=bench@localhost merge -q --no-edit origin/fix/e2e-prerequisites
python_dir="$root/bench/out/e2e-python"
mkdir -p "$python_dir"; ln -sf "$(command -v python3.11 || command -v python3.12 || command -v python3.13)" "$python_dir/python3"
(unset CARGO_TARGET_DIR; (export PATH="$root/bench/guard:$python_dir:$PATH"; cd "$e2e" && AGENT_DESKTOP_E2E_EXCLUSIVE=1 bash tests/e2e/run.sh > "$root/bench/out/e2e-wt-wintitle.log" 2>&1))
status=$?; bench_prune_tree "$e2e"
echo "e2e wintitle+prereq $(git -C "$e2e" rev-parse --short HEAD) exit $status $(grep -cE '^(PASS|ok)' "$root/bench/out/e2e-wt-wintitle.log") pass-lines $(grep -c 'FAIL' "$root/bench/out/e2e-wt-wintitle.log") FAIL-lines; last: $(tail -3 "$root/bench/out/e2e-wt-wintitle.log" | tr '\n' ' ')" | tee -a "$out"
bash bench/run_perf.sh wt-wintitle | tee -a "$out"
