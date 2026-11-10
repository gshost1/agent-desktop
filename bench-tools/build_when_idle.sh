#!/usr/bin/env bash
# usage: build_when_idle.sh <tree>
set -u
idle=0
while [ $idle -lt 2 ]; do
  if pgrep -x cargo >/dev/null || pgrep -x rustc >/dev/null; then idle=0; else idle=$((idle+1)); fi
  sleep 15
done
source /Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost/bench/cargo_env.sh
start=$(date +%s)
bench_build "$1" && echo "built $(cat $1/target/release/agent-desktop.commit) in $(( $(date +%s)-start )) s"
