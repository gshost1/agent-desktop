#!/usr/bin/env bash
# perf-baseline-compare for the open-click branch only, against fork main.
cd "$(dirname "$0")/.."
root="$PWD"
(cd bench/out/wt-click && bash scripts/perf-baseline-compare.sh --base origin/main --out "$root/bench/out/perf-wt-click" > "$root/bench/out/perf-wt-click.log" 2>&1)
echo "wt-click perf exit $?"
