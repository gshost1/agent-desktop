#!/usr/bin/env bash
# A/B for perf/settled-surface-budget: live action latency, then the repo's
# perf-baseline-compare.sh from the fix worktree. Run under the bench lock.
set -u
cd "$(dirname "$0")/.."
bash bench/ab_click.sh bench/out/wt-base/target/release/agent-desktop \
    bench/out/wt-surface/target/release/agent-desktop bench/out/ab_surface.json | tee bench/out/ab_surface.txt
(cd bench/out/wt-surface && bash scripts/perf-baseline-compare.sh --out "$PWD/../perf-surface") 2>&1 | tail -20 | tee -a bench/out/ab_surface.txt
