#!/usr/bin/env bash
# Phase-1 extras after the main matrix: Chrome CLI, background/multi-app, cold start, Chrome model runs.
set -u
cd "$(dirname "$0")/.."
echo "== chrome cli"; python3 bench/chrome_cli.py --runs 9
echo "== background"; python3 bench/background.py --iterations 5
echo "== cold start"; bash bench/cold_start.sh Weather Calculator TextEdit
echo "== chrome model runs"
for r in 1 2 3; do for cfg in a e m; do python3 bench/driver.py --task chrome --config "$cfg" || echo "driver failed"; done; done
