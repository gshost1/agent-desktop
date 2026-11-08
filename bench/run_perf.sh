#!/usr/bin/env bash
# perf-baseline-compare for one fix worktree against fork main: bash bench/run_perf.sh <wt-name>
cd "$(dirname "$0")/.."
root="$PWD"
source bench/cargo_env.sh; unset CARGO_TARGET_DIR
wt="$1"
(cd "bench/out/$wt" && bash scripts/perf-baseline-compare.sh --base origin/main --out "$root/bench/out/perf-$wt" > "$root/bench/out/perf-$wt.log" 2>&1)
status=$?
source bench/cargo_env.sh; bench_prune_tree "bench/out/$wt"
echo "$wt perf exit $status"
