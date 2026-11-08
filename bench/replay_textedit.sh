#!/usr/bin/env bash
# Model-free replay of the fastest passing textedit run (1107223622-cf57) under the bench lock.
# Usage: bash bench/replay_textedit.sh [tree, default bench/out/wt-fork] [run id]
set -u
cd "$(dirname "$0")/.."
owner="perf/speed-cost replay (agent-desktop worktree perf-speed-cost)"
bash bench/lockq.sh take "$owner" 5 >/dev/null
trap 'bash bench/lockq.sh release "$owner"' EXIT
python3 bench/replay.py --run "${2:-1107223622-cf57}" --tree "${1:-bench/out/wt-fork}"
