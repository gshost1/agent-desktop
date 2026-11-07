#!/usr/bin/env bash
# A/B for perf/cold-settle: cold launch + first snapshot, fork main vs the fix.
set -u
cd "$(dirname "$0")/.."
for side in base:bench/out/wt-base cold:bench/out/wt-cold; do
    echo "== ${side%%:*}"
    AD_BIN="${side#*:}/target/release/agent-desktop" bash bench/cold_start.sh Weather Calculator TextEdit
done 2>&1 | tee bench/out/ab_cold.txt
