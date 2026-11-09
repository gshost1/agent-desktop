#!/usr/bin/env bash
# Release-night steps that must not depend on an agent turn. Started detached (setsid) before
# the freeze; it waits for 00:02 PST, then:
#   1. merges origin/main into the release branch (#62) and pushes; aborts on any conflict;
#   2. builds a fresh release binary of main in bench/out/wt-release-main;
#   3. runs the macOS E2E with private steps skipped, under the build and desktop locks;
#   4. runs scripts/perf-baseline-compare.sh against upstream b43ac4f6, same locks.
# Every step logs to bench/out/overnight/ and appends one line to status.txt. Counts and exit
# codes only; the E2E and perf logs stay local.
#   bash bench/overnight.sh <release-worktree> [not-before YYYYmmddHHMM]
set -u
cd "$(dirname "$0")/.."
root="$PWD"
release_wt=$1
not_before=${2:-202611090002}
out="$root/bench/out/overnight"
mkdir -p "$out"
status="$out/status.txt"
note() { echo "$(date '+%Y-%m-%d %H:%M:%S %Z') $*" | tee -a "$status"; }

note "started; waiting for $not_before"
while [ "$(date +%Y%m%d%H%M)" -lt "$not_before" ]; do sleep 60; done

note "step 0: disk guard (free $(df -g "$HOME" | awk 'NR==2{print $4}') GB)"
BENCH_CARGO_TIMEOUT_S=1800 bash bench/serial_cargo.sh python3 bench/prune_merged_targets.py --min-free-gb 30 > "$out/prune.log" 2>&1
note "step 0 exit $? $(grep -E '^prune: removed' "$out/prune.log" | tail -1)"

note "step 1: remerge main into the release branch"
(
    cd "$release_wt" || exit 9
    git fetch -q origin || exit 8
    if git merge-base --is-ancestor origin/main HEAD; then
        echo "already contains origin/main $(git rev-parse --short origin/main)"
        exit 0
    fi
    if ! git merge -q --no-edit origin/main; then
        git merge --abort
        exit 7
    fi
    git push -q origin HEAD:chore/release-1.3.0 || exit 6
) > "$out/remerge.log" 2>&1
note "step 1 exit $? release head $(git -C "$release_wt" rev-parse --short HEAD) main $(git rev-parse --short origin/main)"

note "step 2: fresh release build of main"
source bench/cargo_env.sh
tree="$root/bench/out/wt-release-main"
if [ ! -d "$tree" ]; then git worktree add -q --detach "$tree" origin/main; fi
git -C "$tree" fetch -q origin && git -C "$tree" checkout -q --detach origin/main
bench_build "$tree" > "$out/build.log" 2>&1 && bash bench/check_binary_tree.sh "$tree" >> "$out/build.log" 2>&1
note "step 2 exit $? tree $(git -C "$tree" rev-parse --short HEAD) binary $(cat "$tree/target/release/agent-desktop.commit" 2>/dev/null | cut -c1-8)"

python_dir="$root/bench/out/e2e-python"
mkdir -p "$python_dir"
ln -sf "$(command -v python3.11 || command -v python3.12 || command -v python3.13)" "$python_dir/python3"

note "step 3: E2E (SKIP_PRIVATE, EXCLUSIVE)"
BENCH_CARGO_TIMEOUT_S=7200 bash bench/serial_cargo.sh bash -c "
    bash '$root/bench/lockq.sh' take release-e2e 5 >/dev/null
    export PATH='$root/bench/guard:$python_dir':\"\$PATH\"
    unset CARGO_TARGET_DIR
    cd '$tree' && AGENT_DESKTOP_E2E_SKIP_PRIVATE=1 AGENT_DESKTOP_E2E_EXCLUSIVE=1 bash tests/e2e/run.sh
    code=\$?
    python3 '$root/bench/post_probe_check.py' release-e2e --clean
    bash '$root/bench/lockq.sh' release release-e2e
    exit \$code
" > "$out/e2e.log" 2>&1
e2e=$?
note "step 3 exit $e2e $(grep -E 'passed: [0-9]+ +failed: [0-9]+' "$out/e2e.log" | tail -1 | tr -s ' ') $(grep -E 'private skips: [0-9]+' "$out/e2e.log" | tail -1 | tr -s ' ')"

note "step 4: perf-baseline-compare vs upstream b43ac4f6"
BENCH_CARGO_TIMEOUT_S=7200 bash bench/serial_cargo.sh bash -c "
    bash '$root/bench/lockq.sh' take release-perf 5 >/dev/null
    unset CARGO_TARGET_DIR
    cd '$tree' && bash scripts/perf-baseline-compare.sh --base b43ac4f6 --out '$out/perf-upstream'
    code=\$?
    bash '$root/bench/lockq.sh' release release-perf
    exit \$code
" > "$out/perf.log" 2>&1
note "step 4 exit $?"
note "DONE"
