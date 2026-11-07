#!/usr/bin/env bash
# Phase-1 extras after the main matrix: background/multi-app check and cold-start profile.
set -u
cd "$(dirname "$0")/.."
echo "== background"; python3 bench/background.py --iterations 5
echo "== cold start"; bash bench/cold_start.sh Weather Calculator TextEdit
