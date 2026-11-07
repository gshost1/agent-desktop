#!/usr/bin/env bash
# Runs the repo's perf-baseline-compare in each fix worktree against fork main.
cd "$(dirname "$0")/.."
root="$PWD"
for wt in wt-popup wt-click wt-cold wt-aeprobe wt-outline; do
    d="$root/bench/out/$wt"
    [ -d "$d" ] || continue
    (cd "$d" && bash scripts/perf-baseline-compare.sh --base origin/main --out "$root/bench/out/perf-$wt" > "$root/bench/out/perf-$wt.log" 2>&1)
    echo "$wt exit $? $(grep -iE 'regress|slower|summary' "$root/bench/out/perf-$wt.log" | tail -3 | tr '\n' ' ')"
done
