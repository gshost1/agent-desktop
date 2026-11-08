#!/usr/bin/env bash
# Model-free replay of the fastest passing finder run (1107212636-1afe) under the bench lock.
# Usage: bash bench/replay_finder.sh [tree, default bench/out/wt-fork] [run id]
set -u
cd "$(dirname "$0")/.."
owner="perf/speed-cost replay (agent-desktop worktree perf-speed-cost)"
bash bench/lockq.sh take "$owner" 5 >/dev/null
trap 'bash bench/lockq.sh release "$owner"' EXIT
python3 bench/replay.py --run "${2:-1107212636-1afe}" --tree "${1:-bench/out/wt-fork}"
