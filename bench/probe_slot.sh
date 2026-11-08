#!/usr/bin/env bash
# Runs one live probe in one desktop-lock slot, then the window hygiene check.
#   bash bench/probe_slot.sh <owner> <timeout_s> <output file> <command> [args...]
# The probe counts as failed when it leaves a bench window, a sheet on one, a bench TextEdit
# document or a personal-folder window behind (bench/post_probe_check.py); leftovers are then
# closed so the next slot starts clean.
set -u
owner=$1 limit=$2 out=$3
shift 3
bench="$(cd "$(dirname "$0")" && pwd)"
cd "$bench/.."
bash bench/lockq.sh take "$owner" 5 >/dev/null
bench/out/finder_alert
timeout "$limit" "$@" > "$out" 2>&1
echo "exit $?" >> "$out"
python3 bench/post_probe_check.py "$owner" --clean >> "$out" 2>&1
echo "hygiene exit $?" >> "$out"
bash bench/lockq.sh release "$owner"
tail -4 "$out"
