#!/usr/bin/env bash
# Full model-loop matrix. Needs the bench lock (~/.agent-desktop/bench.lock).
# Rounds interleave tasks and configs so machine drift spreads evenly.
# Usage: bash bench/run_matrix.sh [rounds] [configs] [model]
set -u
cd "$(dirname "$0")/.."
rounds="${1:-3}"; configs="${2:-a b c d e}"; model="${3:-claude-opus-5-5}"
for r in $(seq 1 "$rounds"); do
  for task in weather settings finder textedit calculator; do
    for cfg in $configs; do
      echo "== round $r task $task config $cfg model $model"
      python3 bench/driver.py --task "$task" --config "$cfg" --model "$model" || echo "driver failed"
    done
  done
done
