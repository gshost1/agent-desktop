#!/usr/bin/env bash
# Shared desktop lock with a first-come waiting list, for every bench session.
#
#   ~/.agent-desktop/bench.lock   line 1: holder title, line 2: UTC timestamp
#   ~/.agent-desktop/bench.queue  one waiting title per line, first in line first
#
# A waiter appends its title to the queue. The lock goes to the first title in
# the queue once the lock is free; that waiter writes the lock and removes its
# own line. A releaser that wants the desktop again joins the end of the queue
# like anyone else, so two sessions that both wait alternate.
#
# Usage:
#   bash bench/lockq.sh take "<title>" [poll_s]   join, wait for the turn, hold the lock
#   bash bench/lockq.sh release "<title>"         release the lock if <title> holds it
#   bash bench/lockq.sh join "<title>"            join the queue without waiting
#   bash bench/lockq.sh leave "<title>"           leave the queue
#   bash bench/lockq.sh status                    print the holder and the queue
#
# A lock older than 2 h counts as abandoned. A waiter that uses `take` writes a
# heartbeat every poll; a queued title whose heartbeat is older than 10 min is
# dropped from the queue so a dead waiter cannot block it. Titles queued by hand
# without a heartbeat are never dropped.
set -u
dir="$HOME/.agent-desktop"
lock="$dir/bench.lock"
queue="$dir/bench.queue"
mutex="$dir/bench.queue.mutex"
beats="$dir/bench.queue.beats"
pause="$dir/bench.pause"
mkdir -p "$dir" "$beats"
touch "$queue"

now() { date -u +%s; }

lock_age_s() {
    local stamp
    stamp=$(sed -n 2p "$lock" 2>/dev/null)
    [ -n "$stamp" ] || { echo 0; return; }
    echo $(( $(now) - $(date -u -j -f "%Y-%m-%dT%H:%M:%SZ" "$stamp" +%s 2>/dev/null || now) ))
}

holder() { head -1 "$lock" 2>/dev/null; }

lock_free() {
    [ ! -s "$lock" ] || [ "$(lock_age_s)" -ge 7200 ]
}

beat_file() { printf '%s/%s' "$beats" "$(printf '%s' "$1" | shasum | cut -c1-16)"; }

with_mutex() {
    local tries=0
    until mkdir "$mutex" 2>/dev/null; do
        tries=$((tries + 1))
        if [ "$tries" -gt 300 ]; then rmdir "$mutex" 2>/dev/null; tries=0; fi
        sleep 0.1
    done
    "$@"
    local status=$?
    rmdir "$mutex" 2>/dev/null
    return $status
}

join_unlocked() {
    grep -qxF -- "$1" "$queue" || printf '%s\n' "$1" >> "$queue"
}

leave_unlocked() {
    local rest
    rest=$(grep -vxF -- "$1" "$queue" || true)
    if [ -n "$rest" ]; then printf '%s\n' "$rest" > "$queue.tmp"; else : > "$queue.tmp"; fi
    mv "$queue.tmp" "$queue"
}

prune_unlocked() {
    local title beat
    while IFS= read -r title; do
        [ -n "$title" ] || continue
        beat=$(beat_file "$title")
        if [ -f "$beat" ] && [ $(( $(now) - $(stat -f %m "$beat") )) -gt 600 ]; then
            leave_unlocked "$title"
            rm -f "$beat"
        fi
    done < "$queue"
}

paused() {
    [ -f "$pause" ] && [ $(( $(now) - $(stat -f %m "$pause") )) -lt 1800 ]
}

try_take_unlocked() {
    prune_unlocked
    lock_free || return 1
    paused && return 1
    [ "$(grep -v '^$' "$queue" | head -1)" = "$1" ] || return 1
    printf '%s\n%s\n' "$1" "$(date -u +%FT%TZ)" > "$lock"
    leave_unlocked "$1"
    rm -f "$(beat_file "$1")"
}

release_unlocked() {
    [ "$(holder)" = "$1" ] && rm -f "$lock"
    return 0
}

command="${1:-status}"
title="${2:-}"
case "$command" in
    take)
        [ -n "$title" ] || { echo "take needs a title" >&2; exit 2; }
        poll="${3:-5}"
        with_mutex join_unlocked "$title"
        until with_mutex try_take_unlocked "$title"; do
            touch "$(beat_file "$title")"
            sleep "$poll"
        done
        echo "taken by $title at $(date -u +%FT%TZ)"
        ;;
    release)
        [ -n "$title" ] || { echo "release needs a title" >&2; exit 2; }
        with_mutex release_unlocked "$title"
        ;;
    join)
        [ -n "$title" ] || { echo "join needs a title" >&2; exit 2; }
        with_mutex join_unlocked "$title"
        touch "$(beat_file "$title")"
        ;;
    leave)
        [ -n "$title" ] || { echo "leave needs a title" >&2; exit 2; }
        with_mutex leave_unlocked "$title"
        rm -f "$(beat_file "$title")"
        ;;
    status)
        echo "holder: $(holder) ($(lock_age_s) s)"
        echo "queue:"
        sed 's/^/  /' "$queue"
        ;;
    *)
        echo "unknown command: $command" >&2
        exit 2
        ;;
esac
