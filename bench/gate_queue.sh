#!/usr/bin/env bash
# Runs bench/gate.sh for every "<label>=<rev>" line in bench/out/gate-queue.txt,
# then empties it. Queued as a sh: entry so the gate builds run while this
# session holds the desktop lock, between measured rounds, not under them.
cd "$(dirname "$0")/.."
queue=bench/out/gate-queue.txt
[ -s "$queue" ] || { echo "gate queue empty"; exit 0; }
revs=$(grep -v '^#' "$queue" | tr '\n' ' ')
: > "$queue"
bash bench/gate.sh $revs
