#!/usr/bin/env bash
# What makes headless edits land, measured on the file system and in AX.
# Ideas from bench/oss-survey.md, each measured before anything is built.
#   menu:    a background key chord (agent-desktop press) against an AXPress of
#            the menu item with that shortcut: Finder cmd+shift+n (new folder on
#            disk?) and TextEdit cmd+s (Save sheet appears?)
#   finder:  new folder, then the inline name editor: write AXValue to the
#            focused editor and commit by AXConfirm / Return posted to Finder's
#            pid / AXFocused=false / agent-desktop press return; records the
#            editor's actions, any AXURL up its parents, and what landed on disk
#   owner:   with TextEdit's Save sheet open, which processes own on-screen
#            windows, and whether the panel-service process is running
#   goto:    cmd+shift+g posted to TextEdit's pid, to the panel-service pid, and
#            pressed as the Go to Folder menu item; records surfaces after each
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
front_app=$(echo "$front_before" | python3 -c 'import json,sys; print(json.load(sys.stdin)["app"])')
say "binary $B front $front_before"
panel_pids() { { "$M" pid-of com.apple.appkit.xpc.openAndSavePanelService; pgrep -f openAndSavePanelService | tr '\n' ' '; } 2>/dev/null | tr '\n' ' '; }
surfaces() { "$B" list-surfaces --app TextEdit 2>/dev/null | python3 -c 'import json,sys
try: d=json.load(sys.stdin).get("data")
except ValueError: print("unparsed"); sys.exit()
items = d.get("surfaces", d) if isinstance(d, dict) else d
print([(x.get("type") or x.get("kind"), x.get("title")) for x in (items or [])])'; }

dir="$HOME/agent-desktop-bench/probe-commit-$ts"
mkdir -p "$dir"
open -g "$dir"
sleep 2
"$M" activate "$front_app" >/dev/null 2>&1

count_untitled() { ls "$dir" | grep -c '^untitled folder' ; }
before=$(count_untitled)
"$B" press cmd+shift+n --app Finder > "$out/$ts-menu-chord.json"; sleep 1.2
say "menu finder chord cmd+shift+n: untitled $before -> $(count_untitled) front=$("$M" front | head -c 60)"
"$B" press escape --app Finder >/dev/null 2>&1; sleep 0.4
before=$(count_untitled)
menu_rc=$("$M" menu Finder File "New Folder" 2>&1; echo rc=$?)
sleep 1.2
say "menu finder axpress File>New Folder ($menu_rc): untitled $before -> $(count_untitled) front=$("$M" front | head -c 60)"
"$B" press escape --app Finder >/dev/null 2>&1; sleep 0.4
for f in "$dir"/untitled\ folder*; do [ -d "$f" ] && rmdir "$f"; done

for method in confirm pid-return unfocus ad-return; do
    for mode in background front; do
        name="c-$method-$mode"
        [ "$mode" = front ] && "$M" raise Finder "$(basename "$dir")" >/dev/null 2>&1
        [ "$mode" = background ] && "$M" activate "$front_app" >/dev/null 2>&1
        "$M" menu Finder File "New Folder" >/dev/null 2>&1
        sleep 1.2
        focused=$("$M" focused Finder)
        set_rc=$("$M" set-focused Finder "$name" 2>&1)
        case "$method" in
            confirm) commit_rc=$("$M" perform-focused Finder AXConfirm 2>&1) ;;
            pid-return) commit_rc=$("$M" key Finder 36 2>&1; echo posted) ;;
            unfocus) commit_rc=$("$M" unfocus-focused Finder 2>&1) ;;
            ad-return) commit_rc=$("$B" press return --app Finder | head -c 200) ;;
        esac
        sleep 1.2
        listing=$(ls "$dir" | tr '\n' '|')
        say "finder $method $mode focused=$focused set=$set_rc commit=$commit_rc disk=$listing landed=$([ -d "$dir/$name" ] && echo yes || echo no)"
        "$B" press escape --app Finder >/dev/null 2>&1
        sleep 0.5
        for f in "$dir"/untitled\ folder*; do [ -d "$f" ] && rmdir "$f"; done
    done
done
"$M" close-discard Finder "probe-commit-" >/dev/null 2>&1
mv "$dir" "bench/out/icloud-strays/" 2>/dev/null

ICLOUD="$HOME/Library/Mobile Documents/com~apple~TextEdit/Documents"
"$B" launch TextEdit > /dev/null; sleep 1
"$M" activate "$front_app" >/dev/null 2>&1
"$B" press cmd+n --app TextEdit > /dev/null; sleep 1
"$B" press h --app TextEdit > /dev/null
"$B" press cmd+s --app TextEdit > /dev/null; sleep 2
say "menu textedit chord cmd+s: surfaces=$(surfaces) panel_pids=$(panel_pids)"
"$M" key TextEdit 53 >/dev/null; sleep 0.6
menu_rc=$("$M" menu TextEdit File "Save…" 2>&1; echo rc=$?)
sleep 2
say "menu textedit axpress File>Save… ($menu_rc): surfaces=$(surfaces) panel_pids=$(panel_pids)"
say "owner windows=$("$M" owners | python3 -c 'import json,sys; print(sorted({(w["pid"], w["owner"]) for w in json.load(sys.stdin) if w["layer"] == 0}))')"
say "owner focused=$("$M" focused TextEdit)"

for route in textedit-pid panel-pid menu-item; do
    case "$route" in
        textedit-pid) "$M" key TextEdit 5 cmd,shift ;;
        panel-pid) for p in $(panel_pids); do "$M" keypid "$p" 5 cmd,shift; done ;;
        menu-item) "$M" menu TextEdit Go "Go to Folder…" >/dev/null 2>&1 || "$M" menu TextEdit File "Go to Folder…" >/dev/null 2>&1; echo "menu rc=$?" >> "$log" ;;
    esac
    sleep 1.5
    say "goto $route surfaces=$(surfaces) focused=$("$M" focused TextEdit)"
    "$B" find --app TextEdit --surface sheet --role textfield > "$out/$ts-goto-$route-fields.json" 2>/dev/null
done
"$M" key TextEdit 53 >/dev/null; sleep 0.5
"$M" key TextEdit 53 >/dev/null; sleep 0.5
"$M" close-discard TextEdit Untitled >/dev/null 2>&1
"$M" quit TextEdit
find "$ICLOUD" -maxdepth 1 -newer "$log" -name 'Untitled*' -exec mv {} bench/out/icloud-strays/ \; 2>/dev/null
"$M" activate "$front_app" >/dev/null 2>&1
say "done $log"
