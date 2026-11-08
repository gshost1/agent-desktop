#!/usr/bin/env bash
# What commits a headless edit, measured on the file system.
#   finder:  new folder (cmd+shift+n), set the inline name field's AXValue, then
#            commit by: AXConfirm on the field / Return posted to Finder's pid /
#            AXFocused=false / agent-desktop press return. Records the field's
#            AX actions and which name landed on disk.
#   goto:    TextEdit Save sheet; cmd+shift+g posted to TextEdit's pid, then
#            with TextEdit activated; records the windows and sheets after each.
# Raw AX via bench/out/macctl; strays are moved to bench/out/icloud-strays.
set -u
cd "$(dirname "$0")/.."
B="${1:-bench/out/wt-fork/target/release/agent-desktop}"
[ -x "$B" ] || B=bench/out/wt-base/target/release/agent-desktop
out=bench/out/probe_commit; mkdir -p "$out" bench/out/icloud-strays
export AGENT_DESKTOP_HOME="$PWD/bench/out/home-probe"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"
M=bench/out/macctl
swiftc -O -o "$M" bench/macctl.swift 2>/dev/null || true
ts=$(date +%H%M%S)
log="$out/results-$ts.txt"
say() { echo "$*" | tee -a "$log"; }
front_before=$("$M" front)
say "binary $B front $front_before"

dir="$HOME/agent-desktop-bench/probe-commit-$ts"
mkdir -p "$dir"
open -g "$dir"
sleep 2
for method in confirm pid-return unfocus ad-return; do
    for mode in background front; do
        name="c-$method-$mode"
        [ "$mode" = front ] && "$M" raise Finder "$(basename "$dir")" >/dev/null 2>&1
        [ "$mode" = background ] && "$M" activate "$(echo "$front_before" | python3 -c 'import json,sys; print(json.load(sys.stdin)["app"])')" >/dev/null 2>&1
        "$B" press cmd+shift+n --app Finder > "$out/$ts-$name-new.json"
        sleep 1.2
        focused=$("$M" focused Finder)
        set_rc=$("$M" set-focused Finder "$name" 2>&1)
        case "$method" in
            confirm) commit_rc=$("$M" perform-focused Finder AXConfirm 2>&1) ;;
            pid-return) commit_rc=$("$M" key Finder 36 2>&1; echo posted) ;;
            unfocus) commit_rc=$("$M" unfocus-focused Finder 2>&1) ;;
            ad-return) commit_rc=$("$B" press return --app Finder | head -c 300) ;;
        esac
        sleep 1.2
        listing=$(ls "$dir" | tr '\n' '|')
        say "finder $method $mode focused=$focused set=$set_rc commit=$commit_rc disk=$listing landed=$([ -d "$dir/$name" ] && echo yes || echo no)"
        "$B" press escape --app Finder >/dev/null 2>&1
        sleep 0.5
    done
done
"$M" close-discard Finder "probe-commit-" >/dev/null 2>&1
mv "$dir" "bench/out/icloud-strays/" 2>/dev/null

ICLOUD="$HOME/Library/Mobile Documents/com~apple~TextEdit/Documents"
"$B" launch TextEdit > /dev/null; sleep 1
for mode in pid front; do
    "$B" press cmd+n --app TextEdit > /dev/null; sleep 1
    "$B" press h --app TextEdit > /dev/null
    "$B" press cmd+s --app TextEdit > /dev/null; sleep 2
    before=$("$B" list-surfaces --app TextEdit | head -c 600)
    [ "$mode" = front ] && "$M" activate TextEdit
    "$M" key TextEdit 5 cmd,shift
    sleep 1.5
    after=$("$B" list-surfaces --app TextEdit | head -c 900)
    focused=$("$M" focused TextEdit)
    say "goto $mode surfaces-before=$before"
    say "goto $mode surfaces-after=$after focused=$focused"
    "$M" key TextEdit 53 >/dev/null; sleep 0.5
    "$M" key TextEdit 53 >/dev/null; sleep 0.5
    "$M" close-front-discard TextEdit; sleep 0.5
done
"$M" close-discard TextEdit Untitled >/dev/null 2>&1
"$M" quit TextEdit
find "$ICLOUD" -maxdepth 1 -newer "$log" -name 'Untitled*' -exec mv {} bench/out/icloud-strays/ \; 2>/dev/null
"$M" activate "$(echo "$front_before" | python3 -c 'import json,sys; print(json.load(sys.stdin)["app"])')" >/dev/null 2>&1
say "done $log"
