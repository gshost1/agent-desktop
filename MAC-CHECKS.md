# Mac checks (batch for the Mac worker)

Owner: laptop lead. Each section names the PR, the exact head SHA the steps apply to, the steps and the expected
observations. Run a section only if the PR's current head equals the SHA written here; if it differs, this file is stale
for that PR: skip it and say so. Report per PR: head SHA, each check's PASS/FAIL line, and anything unexpected.

Rules learned from the #58 run (2026-11-10), binding for every section:
- No `rm -rf` in any Mac script: Mac sessions cannot run it. Unlink known files and rmdir known dirs, or leave a listed dir for George.
- Build from a detached worktree under `/tmp` at the SHA (`git -C ~/Projects/agent-desktop worktree add --detach /tmp/<name> <SHA>`); never `git switch` in `~/Projects/agent-desktop`. Wait `while pgrep -x cargo >/dev/null; do sleep 5; done` before building.
- Start with the desktop-space reset (`bench-tools/desktop_space.sh` from `origin/chore/speed-1.4-tasks`; its paths are Mac-local) and wait for the app or fixture window to exist and be front before the first check. A full-screen T3 Code Space made the fixture vanish after ~12 s.
- `find` on a shipped binary omits `bounds` when the match has a ref (`crates/core/src/commands/find_response.rs:61-70`); read bounds with `get <ref> --property bounds`.
- Print full error envelopes, never truncated.
- `wc -w` pads its output on macOS: compare counts with `-eq`, never with `= 1`.
- desktop_space.sh's mktemp must point at the check's own dir, never another session's scratchpad.
- Ship each section as a BUILD block, run outside the desktop lock, and a RUN block, run inside it.
- Every status read must assert that its before-value is non-empty, or HALT; a vacuous PASS is a FAIL.
- No System Events and no UI scripting through osascript, in any section, not even read-only (Automation for System Events is denied on that Mac on purpose). Independent window observations use a CGWindowListCopyWindowInfo helper filtered by the test app's pid (owner pid and layer only; never print other processes' window titles) or agent-desktop's own `list-windows` cross-checked with `kill -0 <pid>`. AppleScript/JXA applets as test subjects are fine if they script no other app.
- Anything a script registers or changes (LaunchServices registrations, apps it launched) is undone in a `trap ... EXIT`, with a line confirming it ran. Quit only apps the script itself launched; an app already running at the start stays running, and the script says so.

Rules for every check: model-free (exact commands, expected output, no judgment calls); no Finder, no TextEdit, no
personal folders (scratch dirs under `/tmp` only); one desktop, 8 GB, so close what you open; a fresh
`AGENT_DESKTOP_HOME` under `/tmp`.

**#34 privacy constraint (hard):** Notification Center on the Mac holds George's real notifications. The #34 check posts
its own notifications from a bench-owned sender, filters the list to its own entries before printing anything, never
prints, logs or returns the title, body or app name of a notification it did not post, reports other notifications only
as counts and key hashes ("N other notifications present, untouched"), never dismisses or acts on a notification it did
not post, and proves it with the foreign count and key-hash set before and after.

## Status

| PR | Row | Head for these steps | State |
|---|---|---|---|
| #80 | SURFACE-10 | `321c63ef` | **RERUN rev 3** on the new head (PASSed on `4ef2c62a`; #110 changed the child walk). CI 13/13, CLEAN; all laptop merges held until #80 lands |
| #58 probe | ACTION-12 | `8959fc5c` | RAN 02:23Z: all 6 ok:true on attempt 1 (D1 did not reproduce); every hit test `unknown`, accepted as verified under the overlay (wrong success); front read AXFocusedApplication -25204. Agent analysing; no further Mac run until a plan is approved |
| #58 (b) hit-test probe | HIT-UNKNOWN-1 | `7de03c99` | RAN 03:09Z: `unknown` did NOT reproduce. The hit test answered every call (uncovered ReachesTarget; covered intercepted_by pfo-cover → occluded refusals). Only unknown: the binary's first-attempt AXFocusedApplication -25204. Agent explaining the difference before any code |
| #58 (b) hit-test repro | HIT-UNKNOWN-1 | `7de03c99` (same probe build) | READY: decides whether the window-list fallback is built; send back `$R/probe.log` + the cleanup lines |
| #58 (a) cap-1 | ACTION-12 | `063d9229` | READY: BUILD + RUN below; CI 13/13 (run 42807214725), gates GREEN 5138/0, draft. Run after #80's rerun (all laptop merges are held for #80) |
| #58 | ACTION-12 | `d57dadfa` | **FAIL 10/17** (two runs, 2026-11-10): early stop never fires on a real Mac, no `occluded` refusal, lock-timeout label on a deadline expiry. Old steps below are superseded; a `#58 probe` section is coming |
| #86 | ADAPTERS-4 + ADAPTERS-1 | `d448d781` | revision 2 ready (full: Part 1, real 1e, Part 2, dense Part D, timing Part T); CI 13/13, CLEAN, draft |
| #88 | LIFECYCLE-2 + LIFECYCLE-12 | `46f397a7` → merged `e6dd96df` | **DONE**: PASS 2×6/6; macos diff vs the merged head 5ebb2548 empty |
| #34 | SURFACE-3 | `e3818161` | CI 27/27 green, CLEAN, ready; rev 2 accepted (1 s Escape rule); HELD for George's go-ahead + Script Editor notification settings |
| #18 | SURFACE-1 | pending | starts after #34 lands |

## #80 SURFACE-10: a focused-surface ref resolves only in the window it was captured from

Head: `321c63efbadb6e393ca609fafa90dcad959ec3c1` (revision 3, rerun on the new head: main `4a41ffc8` merged in, gates GREEN 5136/0, CI 13/13, CLEAN). The Mac PASSed rev 3 on `4ef2c62a`; #110 changed the child walk the observation uses, so it reruns. It merges origin/main `9132aa89` and contains
the docs fix `cf21c2cd`. CI was running at write time: check the head is green before running.

crates/macos changed vs the Part-1-PASS head `2711e496` (main brought in #97 child_read and other macOS files), so Part 1
does NOT carry over; RUN runs Part 1 and Part 2 by default.

Changes from revision 2:
- Part 2's ref comes from `find` WITHOUT `--first` (`--timeout-ms 15000`), which reads list rows (FIND-FIRST-ROWS-1);
  exactly one match is required.
- 1-0 and 2-0 HALT with `capture not unique: <line>` unless the capture is `"unique"`.
- click-status is read via `find --role statictext --native-id click-status --first` → `match.value`, the read
  tests/e2e/lib.sh:175 uses; the check HALTs if the before-value is empty.
- Window counts use `count_of` + `-eq`.
- desktop_space.sh's mktemp is redirected to `$R/space`, and its rm line becomes an echo.
- BUILD runs outside the desktop lock and RUN runs inside it.
- The build uses `CARGO_TARGET_DIR="$R/src/target"`.
- Each run gets its own `$R/run.XXXXXX` (home, docs, logs), so two runs can share one BUILD.

Expected:
- 1-0 and 2-0 read `tokens stripped: N>=1; ... capture="unique"`;
- 1a, 1b, 1d, 1e, 2a, 2b and 2d read PASS;
- 1c and 2c read True.

A HALT line is a setup verdict, not a product FAIL. On main, 1a FAILs and `ran-b` appears; 2a FAILs. Laptop checks:
`bash -n` is clean on both blocks, and grep finds no rm, System Events, osascript, lsappinfo, Finder or TextEdit.

BUILD (no lock; prints `BUILD-OK R=<dir>`):

```bash
#!/usr/bin/env bash
# BUILD for #80 at 321c63efbadb6e393ca609fafa90dcad959ec3c1: run outside the desktop lock
SHA=321c63efbadb6e393ca609fafa90dcad959ec3c1
TAG=ad-s10
set -u
REPO="$HOME/Projects/agent-desktop"
R=$(mktemp -d "${TMPDIR:-/tmp}/$TAG.XXXXXX") || { echo "FAIL-SETUP mktemp"; exit 1; }
mkdir "$R/home" "$R/space" && chmod 700 "$R/home" "$R/space"
echo "scratch dir: $R (nothing is removed; leave it for George)"
git -C "$REPO" fetch -q origin || { echo "FAIL-SETUP fetch"; exit 1; }
[ "$(git -C "$REPO" rev-parse "$SHA^{commit}" 2>/dev/null)" = "$SHA" ] || { echo "FAIL-SETUP $SHA not in the repo"; exit 1; }
DS="$R/desktop_space.sh"
git -C "$REPO" show origin/chore/speed-1.4-tasks:bench-tools/desktop_space.sh \
  | sed -e 's|^r[m] .*AGENT_DESKTOP_HOME.*$|echo "desktop-space home left at $AGENT_DESKTOP_HOME"|' \
        -e 's|mktemp -d /[^)]*|mktemp -d "$SPACE_DIR/space.XXXX"|' > "$DS" \
  || { echo "FAIL-SETUP desktop_space.sh copy"; exit 1; }
if grep -nE '(^|[;&|[:space:]])r[m][[:space:]]' "$DS"; then echo "FAIL-SETUP desktop_space.sh still runs rm"; exit 1; fi
grep -q 'mktemp -d "$SPACE_DIR/space.XXXX"' "$DS" || { echo "FAIL-SETUP desktop_space.sh mktemp line not redirected"; cat "$DS"; exit 1; }
for p in $(grep -oE '/(Users|private)/[^ "()]+' "$DS"); do
  [ -e "$p" ] || { echo "FAIL-SETUP desktop_space.sh path missing: $p"; exit 1; }
done
echo "desktop_space.sh copied to $DS (homes go under $R/space)"
git -C "$REPO" worktree add -q --detach "$R/src" "$SHA" || { echo "FAIL-SETUP worktree"; exit 1; }
while pgrep -x cargo >/dev/null; do sleep 5; done
( cd "$R/src" && CARGO_TARGET_DIR="$R/src/target" cargo build -q --release -p agent-desktop ) || { echo "FAIL-SETUP build"; exit 1; }
B="$R/src/target/release/agent-desktop"
echo "built $("$B" --version) at $(git -C "$R/src" rev-parse HEAD)"
cat > "$R/cgfront.swift" <<'SWIFT'
import CoreGraphics
import Foundation
guard CommandLine.arguments.count == 2, let pid = Int32(CommandLine.arguments[1]) else { exit(2) }
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let list = (CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]]) ?? []
var own = 0
var front: Int32 = -1
for window in list {
    let layer = window[kCGWindowLayer as String] as? Int ?? -1
    let owner = (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value ?? -1
    let alpha = window[kCGWindowAlpha as String] as? Double ?? 0
    let bounds = window[kCGWindowBounds as String] as? [String: Double] ?? [:]
    guard layer == 0, alpha > 0, (bounds["Width"] ?? 0) > 50, (bounds["Height"] ?? 0) > 50 else { continue }
    if front == -1 { front = owner }
    if owner == pid { own += 1 }
}
print("windows=\(own) front=\(front == pid ? "yes" : "no")")
SWIFT
swiftc -O -o "$R/cgfront" "$R/cgfront.swift" 2>"$R/cgfront.log" || { echo "FAIL-SETUP cgfront build"; cat "$R/cgfront.log"; exit 1; }
bash "$R/src/tests/fixture-app/build.sh" "$R/fixture" >"$R/fixture-build.log" 2>&1 || { echo "FAIL-SETUP fixture build"; cat "$R/fixture-build.log"; exit 1; }
echo "BUILD-OK R=$R"
echo "next, with the desktop lock held: bash run.sh $R"
```

RUN (desktop lock held; save as `run.sh`, then `bash run.sh <R>`):

```bash
#!/usr/bin/env bash
# RUN for #80 at 321c63efbadb6e393ca609fafa90dcad959ec3c1: run with the desktop lock held; arguments: the dir BUILD printed, then all (default)
SHA=321c63efbadb6e393ca609fafa90dcad959ec3c1
set -u
R="${1:?usage: run.sh <scratch dir printed by BUILD-OK>}"
B="$R/src/target/release/agent-desktop"
[ -x "$B" ] && [ -x "$R/cgfront" ] && [ -d "$R/fixture/AgentDeskFixture.app" ] || { echo "FAIL-SETUP $R is not a finished BUILD dir"; exit 1; }
[ "$(git -C "$R/src" rev-parse HEAD)" = "$SHA" ] || { echo "FAIL-SETUP $R/src is not at $SHA"; exit 1; }
RUNDIR=$(mktemp -d "$R/run.XXXXXX") || { echo "FAIL-SETUP mktemp"; exit 1; }
mkdir "$RUNDIR/home" && chmod 700 "$RUNDIR/home"
export AGENT_DESKTOP_HOME="$RUNDIR/home"
echo "run dir: $RUNDIR (binary $("$B" --version) at $SHA)"
LAUNCHED_PIDS=""
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
cleanup() {
  echo "== cleanup (trap EXIT)"
  for entry in $LAUNCHED_PIDS; do
    name=${entry%%=*}; name=${name//_/ }; pid=${entry##*=}
    kill -0 "$pid" 2>/dev/null || { echo "cleanup: $name pid $pid already gone"; continue; }
    "$B" close-app "$name" >/dev/null 2>&1
    for _ in $(seq 1 25); do kill -0 "$pid" 2>/dev/null || break; sleep 0.2; done
    kill -0 "$pid" 2>/dev/null && kill "$pid" 2>/dev/null
    for _ in $(seq 1 25); do kill -0 "$pid" 2>/dev/null || break; sleep 0.2; done
    kill -0 "$pid" 2>/dev/null && echo "cleanup: $name pid $pid STILL RUNNING" || echo "cleanup: quit $name pid $pid (launched by this script)"
  done
  "$LSREG" -u "$R/fixture/AgentDeskFixture.app" 2>/dev/null
  echo "cleanup: lsregister -u $R/fixture/AgentDeskFixture.app ran"
  echo "cleanup: done. Left for George: $R (worktree $R/src: git -C ~/Projects/agent-desktop worktree remove --force $R/src)"
}
trap cleanup EXIT
halt() { echo "HALT: $*"; exit 3; }
full() { python3 -c 'import json,sys; d=json.load(sys.stdin); print(json.dumps(d, indent=1)) if d.get("ok") is not True else None'; }
jget() { python3 -c 'import json,sys; d=json.load(sys.stdin); print(eval(sys.argv[1]))' "$1"; }
ref_of() { jget '((d.get("data") or {}).get("match") or {}).get("ref_id","")'; }
count_of() { echo "$1" | wc -w | tr -d ' '; }
status_value() { "$B" find --app "$1" --window-id "$2" --role statictext --native-id "$3" --first | jget '((d.get("data") or {}).get("match") or {}).get("value") or ""'; }

[ -z "$(pgrep -x AgentDeskFixture)" ] || { echo "FAIL-SETUP AgentDeskFixture already running (pid $(pgrep -x AgentDeskFixture)); quit it first"; exit 1; }
if pgrep -x Calculator >/dev/null; then
  echo "desktop-space reset SKIPPED: Calculator was already running (pid $(pgrep -x Calculator)) and stays running; make sure no full-screen app is in front"
else
  SPACE_DIR="$R/space" bash "$R/desktop_space.sh"; echo "desktop-space reset ran (homes under $R/space)"
fi

focused_id() { "$B" list-windows --app "$1" | python3 -c 'import json,sys; print(" ".join(w["id"] for w in json.load(sys.stdin).get("data",[]) if w.get("is_focused")))'; }
front_on() {
  APP="$1"; PID="$2"; WID="$3"
  "$B" focus-window --window-id "$WID" | full
  for _ in $(seq 1 50); do
    CG=$("$R/cgfront" "$PID"); F=$(focused_id "$APP")
    [ "$CG" != "${CG%front=yes}" ] && [ "$F" = "$WID" ] && { echo "front: $APP pid $PID window $WID ($CG)"; return 0; }
    sleep 0.2
  done
  echo "FAIL-SETUP $APP window $WID not front: cg='$CG' focused='$F'"; exit 1
}
win_ids() { "$B" list-windows --app "$1" | python3 -c 'import json,sys; t=sys.argv[1].split("|"); print(" ".join(w["id"] for w in json.load(sys.stdin).get("data",[]) if w.get("title") in t))' "$2"; }
wait_ids() { for _ in $(seq 1 50); do v=$(win_ids "$1" "$2"); [ "$(count_of "$v")" -ge "$3" ] && { echo $v; return; }; sleep 0.2; done; echo ""; }
start_fixture() {
  "$R/fixture/AgentDeskFixture.app/Contents/MacOS/AgentDeskFixture" >"$RUNDIR/fixture.log" 2>&1 &
  FPID=$!; LAUNCHED_PIDS="$LAUNCHED_PIDS AgentDeskFixture=$FPID"
  for _ in $(seq 1 50); do
    M=$("$B" list-windows --app AgentDeskFixture | python3 -c 'import json,sys; print(" ".join(w["id"] for w in json.load(sys.stdin).get("data",[]) if w.get("title")!="Duplicate Window"))')
    [ "$(count_of "$M")" -eq 1 ] && break; sleep 0.2
  done
  echo "fixture pid=$FPID M=$M"; [ "$(count_of "$M")" -eq 1 ] || { echo "FAIL-SETUP fixture window"; exit 1; }
  front_on AgentDeskFixture "$FPID" "$M"
}
PARTS="${2:-all}"
refused() { python3 -c '
import json,sys
d=json.load(sys.stdin); e=d.get("error") or {}
ok = d.get("ok") is False and e.get("code")=="STALE_REF" and "focused_window_changed" in json.dumps(e)
print("PASS" if ok else "FAIL"); print(json.dumps(d, indent=1))'; }
untokenize() { python3 - "$AGENT_DESKTOP_HOME/snapshots/$1/refmap.json" "$2" <<'PY'
import json, os, sys
path, ref = sys.argv[1], "@" + sys.argv[2].rsplit(":", 1)[-1]
data = json.load(open(path)); stripped = 0; capture = "entry-not-found"
def walk(node):
    global stripped, capture
    if isinstance(node, dict):
        if ref in node and isinstance(node[ref], dict):
            capture = node[ref].get("capture", "absent")
        if node.pop("retained_object", None) is not None:
            stripped += 1
        for value in node.values():
            walk(value)
    elif isinstance(node, list):
        for value in node:
            walk(value)
walk(data)
mode = os.stat(path).st_mode & 0o777
with open(path, "w") as out:
    json.dump(data, out)
os.chmod(path, mode)
print("tokens stripped: %d; %s capture=%s" % (stripped, ref, json.dumps(capture)))
PY
}

if [ "$PARTS" = all ]; then
echo "== Part 1: Script Editor (AppKit), two documents"
if pgrep -x "Script Editor" >/dev/null; then
  echo "Part 1 SKIP: Script Editor was already running (pid $(pgrep -x "Script Editor")); it stays running and untouched"
else
  printf 'do shell script "touch %s/ran-a"\n' "$RUNDIR" > "$RUNDIR/a.applescript"
  printf 'do shell script "touch %s/ran-b"\n' "$RUNDIR" > "$RUNDIR/b.applescript"
  open -a "Script Editor" "$RUNDIR/a.applescript"
  for _ in $(seq 1 50); do SEPID=$(pgrep -x "Script Editor"); [ -n "$SEPID" ] && break; sleep 0.2; done
  [ -n "$SEPID" ] || { echo "FAIL-SETUP Script Editor did not start"; exit 1; }
  LAUNCHED_PIDS="$LAUNCHED_PIDS Script_Editor=$SEPID"
  A=$(wait_ids "Script Editor" "a.applescript|a" 1)
  open -a "Script Editor" "$RUNDIR/b.applescript"; W2=$(wait_ids "Script Editor" "b.applescript|b" 1)
  echo "Script Editor pid=$SEPID A=$A B=$W2"; [ -n "$A" ] && [ -n "$W2" ] || { echo "FAIL-SETUP windows"; exit 1; }
  front_on "Script Editor" "$SEPID" "$A"
  F=$("$B" find --app "Script Editor" --window-id "$A" --surface focused --role button --name Run --exact --first)
  REF=$(echo "$F" | ref_of); SNAP=$(echo "$F" | jget '(d.get("data") or {}).get("snapshot_id","")')
  echo "ref=$REF snapshot=$SNAP"; [ -n "$REF" ] || { echo "FAIL-SETUP no Run ref"; echo "$F"; exit 1; }
  echo "1-0 token stripped and capture is unique (expect 'tokens stripped: N>=1; ... capture=\"unique\"'):"; L0=$(untokenize "$SNAP" "$REF"); echo "$L0"
  case "$L0" in *'capture="unique"'*) ;; *) halt "capture not unique: $L0";; esac
  front_on "Script Editor" "$SEPID" "$W2"
  echo "1a click after focus moved to b (expect PASS):"; "$B" click "$REF" | refused
  echo "1b focused find in a while b is focused (expect PASS):"; "$B" find --app "Script Editor" --window-id "$A" --surface focused --role button --name Run --exact --first | refused
  front_on "Script Editor" "$SEPID" "$A"
  echo "1c click after a is focused again (expect True):"; OUT=$("$B" click "$REF"); echo "$OUT" | jget 'd["ok"]'; echo "$OUT" | full
  for _ in $(seq 1 50); do [ -e "$RUNDIR/ran-a" ] && break; sleep 0.2; done
  echo "1d ran-a exists (expect PASS): $([ -e "$RUNDIR/ran-a" ] && echo PASS || echo FAIL)"
  echo "1e ran-b absent (expect PASS): $([ -e "$RUNDIR/ran-b" ] && echo FAIL || echo PASS)"
fi
fi

echo "== Part 2: AgentDeskFixture (SwiftUI)"
start_fixture
BEFORE=$(status_value AgentDeskFixture "$M" click-status); echo "click-status before: '$BEFORE'"
[ -n "$BEFORE" ] || halt "click-status unreadable before the checks"
F=$("$B" find --app AgentDeskFixture --window-id "$M" --surface focused --role button --name primary-button --exact --timeout-ms 15000)
N=$(echo "$F" | jget '(d.get("data") or {}).get("total_matches", -1)')
REF2=$(echo "$F" | jget '(((d.get("data") or {}).get("matches") or [{}])[0]).get("ref_id","")')
SNAP2=$(echo "$F" | jget '(d.get("data") or {}).get("snapshot_id","")')
echo "matches=$N ref=$REF2 snapshot=$SNAP2"
[ "$N" = 1 ] && [ -n "$REF2" ] || { echo "$F" | full; halt "primary-button: expected 1 match with a ref, got $N"; }
echo "2-0 token stripped and capture is unique (expect 'tokens stripped: N>=1; ... capture=\"unique\"'):"; L0=$(untokenize "$SNAP2" "$REF2"); echo "$L0"
case "$L0" in *'capture="unique"'*) ;; *) halt "capture not unique: $L0";; esac
O=$("$B" find --app AgentDeskFixture --window-id "$M" --role button --name open-duplicate-windows --exact --first | ref_of)
"$B" click "$O" | full; D=$(wait_ids AgentDeskFixture "Duplicate Window" 2); D1=${D%% *}; echo "duplicates: $D"
[ -n "$D1" ] || { echo "FAIL-SETUP duplicate windows"; exit 1; }
front_on AgentDeskFixture "$FPID" "$D1"
echo "2a click after focus moved to a duplicate window (expect PASS):"; "$B" click "$REF2" | refused
S2B=$(status_value AgentDeskFixture "$M" click-status)
echo "2b click-status unchanged (expect PASS): $([ "$S2B" = "$BEFORE" ] && echo PASS || echo FAIL) (now '$S2B')"
front_on AgentDeskFixture "$FPID" "$M"
echo "2c click after main window is focused again (expect True):"; OUT=$("$B" click "$REF2"); echo "$OUT" | jget 'd["ok"]'; echo "$OUT" | full
for _ in $(seq 1 25); do S2D=$(status_value AgentDeskFixture "$M" click-status); [ -n "$S2D" ] && [ "$S2D" != "$BEFORE" ] && break; sleep 0.2; done
echo "2d click-status changed (expect PASS): $([ -n "$S2D" ] && [ "$S2D" != "$BEFORE" ] && echo PASS || echo FAIL) (now '$S2D')"
C=$("$B" find --app AgentDeskFixture --window-id "$M" --role button --name close-duplicate-windows --exact --first | ref_of)
"$B" click "$C" | full
echo "== checks done"
```

## #58 ACTION-12: headed hover/drag stops after a raise that changed the front window

Head: `d57dadfab3284ef4234d05c68217fd789c255265`. Uses the e2e AgentDeskFixture, Calculator and a floating-panel helper
under `/tmp/pfo`. Run in a terminal that already has Accessibility trust (the one used for `tests/e2e/run.sh`). The
Python was dry-run against a stub binary on the laptop (17 checks); not yet run on a real Mac.

The earlier Mac run (AgentDeskFixture with Font Book in front) never covered the target, so it did not exercise the raise. This check covers the fixture's `hover-target` with a floating panel from a throwaway helper in `/tmp/pfo`, which stays above the fixture after the raise. It uses only the e2e fixture, Calculator (used by `tests/e2e/scenarios/os_surfaces.sh`) and `/tmp/pfo`. Run it on the Mac in a terminal that already has Accessibility trust (the one used for `tests/e2e/run.sh`). Each check prints `PASS` or `FAIL` with the command's code and time.

| Case | Setup | Expected |
|---|---|---|
| A ×5 | fixture in front, target uncovered | `ok: true`, `hover-status` = `hovered` |
| B ×5 | Calculator in front, target uncovered | `ok: true`, `hover-status` = `hovered` (a run flagged `ASYNC-RAISE-RISK` is the accepted risk below) |
| D ×2 | fixture in front, target covered | `TIMEOUT` after ≥ 2.5 s of a 3 s budget, `details.preparatory_focus` set (no raise over another window, so it keeps waiting) |
| C ×3 | Calculator in front, target covered | `ACTION_FAILED` (occluded) in < 2.5 s of a 5 s budget, `details.preparatory_focus` set, fixture now in front (on `main` the unit test shows it raising every attempt, then `TIMEOUT`) |
| E ×2 | Calculator in front, drag from `context-target` onto the covered `hover-target` | same as C |

```bash
cd ~/Projects/agent-desktop
git status --short                      # expect: no output (stop if the checkout is dirty)
git fetch origin && git switch fix/pointer-focus-once && git pull --ff-only
git log -1 --format=%H                  # expect: d57dadfab3284ef4234d05c68217fd789c255265
cargo test -p agent-desktop-core --lib focus_phase   # expect: test result: ok. 14 passed; 0 failed
cargo test -p agent-desktop-macos       # expect: every "test result:" line says 0 failed
cargo build --release -p agent-desktop
pgrep -x AgentDeskFixture || echo none  # expect: none
pgrep -x Calculator || echo none        # expect: none
rm -rf /tmp/pfo && mkdir -p /tmp/pfo/fixture
bash tests/fixture-app/build.sh /tmp/pfo/fixture
cat > /tmp/pfo/overlay.swift <<'EOF'
import AppKit

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let numbers = CommandLine.arguments.dropFirst().compactMap { Double($0) }
guard numbers.count == 4, let primary = NSScreen.screens.first else { exit(2) }
let frame = NSRect(x: numbers[0], y: primary.frame.height - numbers[1] - numbers[3],
                   width: numbers[2], height: numbers[3])
let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel],
                    backing: .buffered, defer: false)
panel.level = .floating
panel.hidesOnDeactivate = false
panel.backgroundColor = .systemRed
let cover = NSButton(title: "pfo-cover", target: nil, action: nil)
cover.frame = NSRect(origin: .zero, size: frame.size)
panel.contentView?.addSubview(cover)
panel.orderFrontRegardless()
application.run()
EOF
swiftc -O -o /tmp/pfo/overlay /tmp/pfo/overlay.swift -framework AppKit
cat > /tmp/pfo/check.py <<'EOF'
import json, os, subprocess, sys, time

BIN = os.environ.get("PFO_BIN", os.path.expanduser("~/Projects/agent-desktop/target/release/agent-desktop"))
APP = "AgentDeskFixture"
OTHER = "Calculator"
overlay = None
failures = 0
checks = 0


def run(*args):
    started = time.monotonic()
    proc = subprocess.run([BIN, *args], capture_output=True, text=True, timeout=60)
    elapsed = int((time.monotonic() - started) * 1000)
    try:
        body = json.loads(proc.stdout)
    except ValueError:
        body = {"ok": None, "raw": (proc.stdout + proc.stderr)[:600]}
    return body, elapsed


def cleanup():
    if overlay is not None:
        overlay.kill()
    run("close-app", OTHER)
    run("close-app", APP, "--force")


def setup_fail(message, body=None):
    print(f"SETUP FAIL: {message} {json.dumps(body)[:600] if body else ''}")
    cleanup()
    sys.exit(1)


def match_of(body):
    return (body.get("data") or {}).get("match") or {}


def find_button(name):
    body = None
    for _ in range(100):
        body, _ = run("find", "--app", APP, "--role", "button", "--name", name, "--exact", "--first")
        match = match_of(body)
        if body.get("ok") is True and match.get("ref_id") and match.get("bounds"):
            return match
        time.sleep(0.2)
    setup_fail(f"button {name} not found", body)


def readout(name):
    body, _ = run("find", "--app", APP, "--role", "statictext", "--native-id", name, "--first")
    return match_of(body).get("value")


def front_name():
    asn = subprocess.run(["lsappinfo", "front"], capture_output=True, text=True).stdout.strip()
    return subprocess.run(["lsappinfo", "info", "-only", "name", asn], capture_output=True, text=True).stdout


def bring_front(name):
    if name == APP:
        run("focus-window", "--app", APP)
    else:
        subprocess.run(["open", "-a", name])
    for _ in range(50):
        if f'"{name}"' in front_name():
            time.sleep(0.3)
            return
        time.sleep(0.1)
    setup_fail(f"{name} did not come to the front: {front_name()}")


def record(label, passed, detail):
    global failures, checks
    checks += 1
    failures += 0 if passed else 1
    print(f"{'PASS' if passed else 'FAIL'} {label}: {detail}")


def error_of(body):
    return body.get("error") or {}


def hovered_within(seconds):
    for _ in range(int(seconds * 5)):
        if readout("hover-status") == "hovered":
            return True
        time.sleep(0.2)
    return False


def uncovered_hover(case, front, runs, hover_ref):
    for index in range(1, runs + 1):
        run("--headed", "mouse-move", "--xy", "20,20")
        bring_front(front)
        body, elapsed = run("--headed", "hover", hover_ref, "--timeout-ms", "3000")
        error = error_of(body)
        detail = f"ok={body.get('ok')} code={error.get('code')} ms={elapsed}"
        if body.get("ok") is not True and "preparatory_focus" in json.dumps(error):
            detail += " ASYNC-RAISE-RISK"
        record(f"{case} run {index}", body.get("ok") is True and hovered_within(3), detail + f" hover-status={readout('hover-status')}")


def covered(case, front, runs, command, expect_code, ms_check):
    for index in range(1, runs + 1):
        bring_front(front)
        body, elapsed = run("--headed", *command)
        error = error_of(body)
        text = json.dumps(error)
        passed = (body.get("ok") is False and error.get("code") == expect_code
                  and "occluded" in text and ms_check(elapsed))
        passed = passed and "preparatory_focus" in (error.get("details") or {})
        if expect_code == "ACTION_FAILED":
            for _ in range(20):
                if f'"{APP}"' in front_name():
                    break
                time.sleep(0.1)
            raised = f'"{APP}"' in front_name()
            passed = passed and raised
            text = f"raised={raised} " + text
        record(f"{case} run {index}", passed, f"ok={body.get('ok')} code={error.get('code')} ms={elapsed} {text[:300]}")


def overlaps(a, b):
    return not (a["x"] + a["width"] <= b["x"] or b["x"] + b["width"] <= a["x"]
                or a["y"] + a["height"] <= b["y"] or b["y"] + b["height"] <= a["y"])


def main():
    global overlay
    run("move-window", "--app", APP, "--x", "40", "--y", "60")
    run("resize-window", "--app", APP, "--width", "1000", "--height", "700")
    bring_front(APP)
    for _ in range(3):
        body, _ = run("scroll-to", find_button("hover-target")["ref_id"])
        if body.get("ok") is True:
            break
        time.sleep(0.5)
    time.sleep(0.5)
    hover = find_button("hover-target")
    context = find_button("context-target")
    box = hover["bounds"]
    cover = {"x": box["x"] - 6, "y": box["y"] - 6, "width": box["width"] + 12, "height": box["height"] + 12}
    if overlaps(cover, context["bounds"]):
        setup_fail(f"overlay would cover context-target: hover={box} context={context['bounds']}")
    print(f"hover-target {hover['ref_id']} bounds={box}; context-target {context['ref_id']}")

    uncovered_hover("A already-front uncovered hover (expect ok, hovered)", APP, 5, hover["ref_id"])
    uncovered_hover("B behind-Calculator uncovered hover (expect ok, hovered)", OTHER, 5, hover["ref_id"])

    run("--headed", "mouse-move", "--xy", "20,20")
    overlay = subprocess.Popen([os.environ.get("PFO_OVERLAY", "/tmp/pfo/overlay"), str(cover["x"]), str(cover["y"]),
                                str(cover["width"]), str(cover["height"])])
    time.sleep(1.5)
    if overlay.poll() is not None:
        setup_fail(f"overlay exited with {overlay.returncode}")

    covered("D already-front covered hover (expect TIMEOUT, >=2500 ms, preparatory_focus)", APP, 2,
            ["hover", hover["ref_id"], "--timeout-ms", "3000"], "TIMEOUT", lambda ms: ms >= 2500)
    covered("C behind-Calculator covered hover (expect ACTION_FAILED, <2500 ms, preparatory_focus, fixture raised)", OTHER, 3,
            ["hover", hover["ref_id"], "--timeout-ms", "5000"], "ACTION_FAILED", lambda ms: ms < 2500)
    covered("E behind-Calculator drag onto covered drop (expect ACTION_FAILED, <2500 ms, preparatory_focus, fixture raised)", OTHER, 2,
            ["drag", "--from", context["ref_id"], "--to", hover["ref_id"], "--timeout-ms", "5000"],
            "ACTION_FAILED", lambda ms: ms < 2500)

    cleanup()
    print(f"RESULT: {'ALL PASS' if failures == 0 else f'{failures} FAIL'} ({checks} checks)")
    sys.exit(0 if failures == 0 else 1)


main()
EOF
open /tmp/pfo/fixture/AgentDeskFixture.app
python3 /tmp/pfo/check.py               # expect: 17 PASS lines, then: RESULT: ALL PASS (17 checks)
pgrep -x AgentDeskFixture || echo none  # expect: none
pgrep -x Calculator || echo none        # expect: none
```

Report the last line (`RESULT: ...`) and any `FAIL` or `SETUP FAIL` line verbatim on this PR or to the control thread.

## #34 SURFACE-3: notification keys (accepted; HELD for George's go-ahead)

Head: `e3818161e6201c24989de964ad567aecb0056d95` (agent will not move it unless main conflicts). CI at write time: 23
pass, 4 pending, none red. Revision 2 (2026-11-10), with control's five changes.

**agent-desktop itself uses System Events (decided: accept the race for this check; product fix queued as MAC-SE-1).** On a closed Notification Center, every `--headed`
notification command opens it through System Events UI scripting: `crates/macos/src/notifications/nc_session.rs:277`
(on origin/main too) runs `osascript` → `tell application "System Events" ... click menu bar item ... "Clock"`. It skips
that when the center is already open (`nc_session.rs:52-60`). The script works around it. It prints `ACTION open Notification
Center by clicking the date and time in the menu bar` and waits up to 120 s. Every list runs headless, which never opens
the center and answers `POLICY_DENIED` when it is closed. A `--headed` mutation is sent only right after a headless list
proved the center open. **Residual race (accepted by control 2026-11-10):** if the panel closes in the ~0.5 s between that check and the
headed call, the adapter's osascript runs; System Events Automation is denied on that Mac on purpose, so it fails, and
the script treats the failed call as HALT (error code only, no retry). The adapter change that removes this path is
ledger row MAC-SE-1 in TASKS.md. If that
is unacceptable, this check cannot run until the adapter opens the center another way (a product change; the macOS
permission preflight in `crates/macos/src/system/permissions/` also depends on System Events Automation).

The five changes:
1. **State dir.** A fresh `mkdtemp` under `/tmp` is passed as `AGENT_DESKTOP_HOME`. Every inherited `AGENT_DESKTOP_*`
   variable is stripped, AGENT_DESKTOP_SESSION included. The directory is removed in the outermost `finally`, with an
   `UNDO ...` line. The agent's one-line answer: without `--trace` and without a trace-enabled session
   (`crates/core/src/trace/mod.rs:26-52`), list, dismiss and action write no title, body or app_name under the state
   root. Refmaps come only from snapshot, which the script never runs. The cursor overlay holds coordinates only.
2. **Catch-all.** Any unexpected exception prints only `ERROR <TypeName>`, still cleans up, closes the center and removes
   the state dir, then exits 1. Error envelopes print only code, delivery, retry, recovery.strategy, details.kind and
   details.reason, each matched against `^[A-Za-z0-9_.:-]{1,64}$`. Messages are never printed.
3. **`--app` on the stale-index `notification-action` call,** along with `--key` and `--expected-title`.
4. **Stop on first failure.** The first FAIL, a wait timeout or a closed center raises Halt. All further posts, dismisses
   and actions are refused, the script prints `STOP ...`, and cleanup dismisses only nonce entries by key with the title
   and app guards.
5. **Close the panel.** agent-desktop's own `press escape` (headless, unscoped), the key its adapter's `close_nc` sends
   (`nc_session.rs:304-318`). It is sent only when the script's own headless list saw the center open less than 1 s
   earlier (control's rule, 2026-11-10); otherwise the script sends nothing and prints the NOTE. Closure is verified by a headless list that
   must answer POLICY_DENIED within 5 s: `PASS notification center closed`, else `NOTE notification center left open`.

Checked against the real macOS code path, not the fake center:
- The list data is `{count, notifications:[{index, app_name, title, body?, actions?, key}]}`.
- macOS takes app, title and body from the row's AXDescription (`crates/macos/src/notifications/scan.rs`
  `parse_row_description`). With one text after the app name, the title becomes the app name.
- So an entry is the bench's when the nonce is in its title OR its body. A nonce entry of unknown kind is still cleaned
  up.

Fake-center results, run by the agent with foreign PRIVATE-*/SecretApp entries, foreign text in error messages, and a
log of every mutation:
- fixed: PASS.
- index-only addressing: one FAIL, then STOP; only bench cleanup follows.
- exception injected: only `ERROR AttributeError`.
- 0 foreign strings printed in every case.

Laptop lead's notes:
- **Escape:** `press escape` is unscoped, so it reaches the frontmost app if the center is not what receives it. The
  script sends it only when a headless list just saw the center open.
- **desktop_space.sh:** it hard-codes Mac-local paths (the bench-v130 binary, macctl) and silently does nothing if they
  are missing.
- **Undo:** the run block's `trap undo EXIT` removes the worktree and both /tmp files. The Script Editor notification
  settings (Allow, Banners, grouping Off) are George's to set and to undo; the script changes no settings.
- **Syntax:** bash -n and py_compile are clean on the laptop.

Operator: when the script prints ACTION, click the menu-bar clock once and touch nothing until it ends (3-4 min).
Expected output:
- every line PASS, then `PASS notification center closed`, `RESULT PASS`, `UNDO removed agent-desktop state dir ...`
  and `exit 0`;
- the trap's `UNDO removed worktree ...` and `UNDO removed /tmp/adbench-34-check.py /tmp/adbench-34-space.sh`.

Exit 1 means FAIL or ERROR; exit 3 means INCONCLUSIVE.

Run block (one paste):

```bash
set -u
SHA=e3818161e6201c24989de964ad567aecb0056d95
R=~/Projects/agent-desktop; W=/tmp/adbench-34; PY=/tmp/adbench-34-check.py; SP=/tmp/adbench-34-space.sh
undo() { [ -d "$W" ] && git -C "$R" worktree remove --force "$W" && echo "UNDO removed worktree $W"; rm -f "$PY" "$SP" && echo "UNDO removed $PY $SP"; }
trap undo EXIT
cat > "$PY" <<'ADBENCH_PY_EOF'
#!/usr/bin/env python3
"""Mac check for agent-desktop notification keys (fork PR #34).

Posts its own notifications with a random nonce in the title and body and acts
only on entries that carry that nonce. Raw agent-desktop output stays in memory
and goes straight through bench_view(); nothing about another notification is
printed or written anywhere except a count and a hash of its opaque key stems.

agent-desktop opens a closed Notification Center through System Events, so the
operator opens it by hand first. Lists run headless, which can never open it,
and a --headed mutation is sent only right after a headless list proves the
center is open, so agent-desktop's open path never runs.

Usage: AD=/path/to/agent-desktop python3 -I adbench_notification_keys.py
Exit: 0 pass, 1 fail or error, 3 inconclusive.
"""
import hashlib
import json
import os
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
import time

AD = os.environ.get("AD", "agent-desktop")
NONCE = "adbench-" + secrets.token_hex(6)
KIND_NAMES = ("twin", "solo", "newer")
FAILS = []
INCONCLUSIVE = []
ENV = {name: value for name, value in os.environ.items() if not name.startswith("AGENT_DESKTOP_")}
STATE = {"cleaning": False}
WORD = re.compile(r"^[A-Za-z0-9_.:-]{1,64}$")


class Halt(Exception):
    """A check failed or the center closed: no further scenario mutations."""


def notification(kind):
    return (f"{NONCE}-{kind}", f"{NONCE} {kind} body")


def say(line):
    print(line, flush=True)


def check(ok, label):
    say(("PASS " if ok else "FAIL ") + label)
    if not ok:
        FAILS.append(label)
        if not STATE["cleaning"]:
            raise Halt()
    return ok


def word(value):
    return value if isinstance(value, str) and WORD.match(value) else None


def run(*args, headed=False):
    """Runs agent-desktop and returns only a sanitized summary of the envelope."""
    argv = [AD, "--headed", *args] if headed else [AD, *args]
    try:
        proc = subprocess.run(argv, capture_output=True, text=True, timeout=180, env=ENV)
    except subprocess.TimeoutExpired:
        return {"ok": False, "code": "SCRIPT_TIMEOUT"}
    try:
        envelope = json.loads(proc.stdout)
    except ValueError:
        return {"ok": False, "code": "UNPARSEABLE_OUTPUT", "exit": proc.returncode}
    if envelope.get("ok"):
        return {"ok": True, "data": envelope.get("data") or {}}
    error = envelope.get("error") or {}
    details = error.get("details") if isinstance(error.get("details"), dict) else {}
    disposition = error.get("disposition") if isinstance(error.get("disposition"), dict) else {}
    recovery = error.get("recovery") if isinstance(error.get("recovery"), dict) else {}
    return {
        "ok": False,
        "code": word(error.get("code")),
        "delivery": word(disposition.get("delivery")),
        "retry": word(disposition.get("retry")),
        "recovery": word(recovery.get("strategy")),
        "kind": word(details.get("kind")),
        "reason": word(details.get("reason")),
        "exit": proc.returncode,
    }


def outcome(result):
    if result["ok"]:
        return "ok"
    fields = ("code", "delivery", "retry", "recovery", "kind", "reason", "exit")
    return " ".join(f"{name}={result.get(name)}" for name in fields if result.get(name) is not None)


def parse_key(key):
    """Splits a key into (digest, occurrence, twins); None when it is not one."""
    try:
        digest, counts = key.split("-", 1)
        occurrence, twins = counts.split("of", 1)
        return digest, int(occurrence), int(twins)
    except (AttributeError, ValueError):
        return None


def stem(key):
    """The digest and occurrence of a key, which an identical newcomer does not change."""
    parsed = parse_key(key)
    return f"{parsed[0]}-{parsed[1]}" if parsed else "unparsed"


def bench_kind(entry):
    """The bench kind of an entry that carries the nonce, or None for any other entry."""
    title = entry.get("title") if isinstance(entry.get("title"), str) else ""
    body = entry.get("body") if isinstance(entry.get("body"), str) else ""
    if NONCE not in title and NONCE not in body:
        return None
    for name in KIND_NAMES:
        if title == f"{NONCE}-{name}" or body.startswith(f"{NONCE} {name} "):
            return name
    return "unknown"


def keys_hash(keys):
    return hashlib.sha256("\n".join(sorted(keys)).encode()).hexdigest()[:12]


def bench_view():
    """Lists headless once: the bench's own entries, and only key stems for the rest.

    A headless list never opens the center; it fails POLICY_DENIED when it is closed.
    """
    result = run("list-notifications")
    if not result["ok"]:
        return None, result
    ours, foreign = [], []
    for entry in result["data"].get("notifications") or []:
        kind = bench_kind(entry)
        if kind is None:
            foreign.append(stem(entry.get("key")))
            continue
        ours.append(
            {
                "index": entry.get("index"),
                "key": entry.get("key"),
                "kind": kind,
                "title": entry.get("title"),
                "app": entry.get("app_name"),
            }
        )
    return {"ours": ours, "foreign": foreign}, result


def center_open():
    view, result = bench_view()
    if view is not None:
        return True
    if result.get("code") == "POLICY_DENIED":
        return False
    return None


def kind(view, name):
    return sorted((e for e in view["ours"] if e["kind"] == name), key=lambda e: e["index"])


def show(view, label):
    shown = ", ".join(f"{e['kind']}@{e['index']}={e['key']}" for e in sorted(view["ours"], key=lambda e: e["index"]))
    say(f"     {label}: bench [{shown}]; {len(view['foreign'])} other present (keys hash {keys_hash(view['foreign'])})")


def guard_mutation():
    if FAILS and not STATE["cleaning"]:
        raise Halt()
    if center_open() is not True:
        say("NOTE notification center is not open; no headed call is sent, so agent-desktop does not open it")
        INCONCLUSIVE.append("center closed during the run")
        raise Halt()


def post(kind_name):
    guard_mutation()
    title, body = notification(kind_name)
    subprocess.run(
        ["osascript", "-e", "on run argv", "-e",
         "display notification (item 2 of argv) with title (item 1 of argv)",
         "-e", "end run", title, body],
        check=True,
        capture_output=True,
        env=ENV,
    )


def dismiss(entry, index=None):
    guard_mutation()
    args = ["dismiss-notification"]
    if index is not None:
        args.append(str(index))
    args += ["--key", entry["key"], "--expected-title", entry["title"]]
    if entry.get("app"):
        args += ["--app", entry["app"]]
    return run(*args, headed=True)


def stale_action(entry, index):
    guard_mutation()
    args = ["notification-action", str(index), NONCE + "-no-such-action",
            "--key", entry["key"], "--expected-title", entry["title"]]
    if entry.get("app"):
        args += ["--app", entry["app"]]
    return run(*args, headed=True)


def wait_for(predicate, label, timeout_s=40):
    deadline = time.monotonic() + timeout_s
    last = None
    while time.monotonic() < deadline:
        view, result = bench_view()
        if view is None:
            last = outcome(result)
        elif predicate(view):
            return view
        time.sleep(2)
    say(f"INCONCLUSIVE {label} did not appear within {timeout_s}s (last list error: {last})")
    INCONCLUSIVE.append(label)
    raise Halt()


def refusal(result):
    return (not result["ok"]) and result.get("code") == "NOTIFICATION_NOT_FOUND" and result.get("delivery") == "not_delivered"


def scenario():
    post("twin")
    time.sleep(1.5)
    post("twin")
    first = wait_for(lambda v: len(kind(v, "twin")) >= 2, "two identical bench notifications")
    show(first, "after two twins")
    twins = kind(first, "twin")
    check(len(twins) == 2, f"exactly two twins listed (saw {len(twins)})")
    parsed = parse_key(twins[0]["key"])
    check(parsed is not None, f"keys have the digest-NofM form ({twins[0]['key']})")
    digest = parsed[0]
    check({e["key"] for e in twins} == {f"{digest}-1of2", f"{digest}-2of2"}, "twins share one digest and are keyed 1of2 and 2of2")
    oldest = next(e for e in twins if e["key"] == f"{digest}-1of2")
    check(oldest["index"] == max(e["index"] for e in twins), "the 1of2 twin is the lower (older) entry")

    post("twin")
    second = wait_for(lambda v: len(kind(v, "twin")) >= 3, "third identical bench notification")
    show(second, "after third twin")
    twins3 = kind(second, "twin")
    check({e["key"] for e in twins3} == {f"{digest}-{n}of3" for n in (1, 2, 3)}, "the identical newcomer took 3of3; the older twins kept their numbers")
    newest = next(e for e in twins3 if e["key"] == f"{digest}-3of3")
    check(newest["index"] == min(e["index"] for e in twins3), "the 3of3 twin is listed on top (newest first)")
    oldest_now = next(e for e in twins3 if e["key"] == f"{digest}-1of3")
    check(oldest_now["index"] != oldest["index"], f"the oldest twin moved from index {oldest['index']} to {oldest_now['index']}")
    second_oldest = next(e for e in twins3 if e["key"] == f"{digest}-2of3")
    occupant = next((e for e in second["ours"] if e["index"] == oldest["index"]), None)
    check(occupant is not None, f"index {oldest['index']} now holds a bench entry, so the stale-index call can only reach the bench's own")

    stale = dismiss(oldest, index=oldest["index"])
    check(refusal(stale), f"stale index {oldest['index']} with key {oldest['key']} is refused NOTIFICATION_NOT_FOUND/not_delivered (got {outcome(stale)})")
    after_stale = wait_for(lambda v: True, "list after the refused dismiss")
    check(len(kind(after_stale, "twin")) == 3, "the refused dismiss closed no twin (still 3)")

    keyed = dismiss(oldest)
    check(keyed["ok"], f"dismiss --key {oldest['key']} succeeds (got {outcome(keyed)})")
    third = wait_for(lambda v: len(kind(v, "twin")) <= 2, "twin count back to two")
    show(third, "after keyed dismiss")
    remaining = {e["key"] for e in kind(third, "twin")}
    check(remaining == {f"{digest}-1of2", f"{digest}-2of2"}, "two twins remain, renumbered 1of2 and 2of2 (which identical twin closed is not observable)")

    left = dismiss(second_oldest)
    check(refusal(left), f"key {second_oldest['key']} from before a twin left is refused NOTIFICATION_NOT_FOUND/not_delivered (got {outcome(left)})")
    after_left = wait_for(lambda v: True, "list after the refused twin-left dismiss")
    check(len(kind(after_left, "twin")) == 2, "the refused twin-left dismiss closed no twin (still 2)")

    post("solo")
    with_solo = wait_for(lambda v: len(kind(v, "solo")) == 1, "distinct bench notification")
    solo = kind(with_solo, "solo")[0]
    post("newer")
    with_newer = wait_for(lambda v: len(kind(v, "newer")) == 1, "newer distinct bench notification")
    show(with_newer, "after solo and newer")
    solo_now = next((e for e in kind(with_newer, "solo") if e["key"] == solo["key"]), None)
    check(solo_now is not None, "the solo entry kept its key after a newer arrival")
    check(solo_now["index"] != solo["index"], f"the solo entry moved from index {solo['index']} to {solo_now['index']}")
    occupant = next((e for e in with_newer["ours"] if e["index"] == solo["index"]), None)
    check(occupant is not None, f"index {solo['index']} now holds a bench entry")
    stale = dismiss(solo, index=solo["index"])
    check(refusal(stale), f"stale solo index {solo['index']} is refused NOTIFICATION_NOT_FOUND/not_delivered (got {outcome(stale)})")
    stale_pressed = stale_action(solo, solo["index"])
    check(refusal(stale_pressed), f"stale-index notification-action is refused NOTIFICATION_NOT_FOUND/not_delivered (got {outcome(stale_pressed)})")
    exact = dismiss(solo_now, index=solo_now["index"])
    check(exact["ok"], f"dismiss {solo_now['index']} --key {solo['key']} succeeds (got {outcome(exact)})")
    final = wait_for(lambda v: not kind(v, "solo"), "solo entry gone")
    show(final, "after solo dismiss")
    check(len(kind(final, "newer")) == 1 and len(kind(final, "twin")) == 2, "only the solo entry closed: newer and both twins remain")


def cleanup():
    """Dismisses only bench entries, each by its key with the title and app guards."""
    STATE["cleaning"] = True
    for _ in range(4):
        view, result = bench_view()
        if view is None:
            say(f"NOTE cleanup cannot list ({outcome(result)}); bench notifications carry nonce {NONCE}")
            return None
        if not view["ours"]:
            return view
        for entry in view["ours"]:
            try:
                dismiss(entry)
            except Halt:
                return None
    view, _ = bench_view()
    return view


def close_center():
    """Closes the center with agent-desktop's own Escape, the key its adapter uses."""
    seen_at = time.monotonic()
    if center_open() is not True:
        say("PASS notification center closed")
        return
    if time.monotonic() - seen_at > 1.0:
        say("NOTE notification center left open (open state not confirmed within the last second; Escape not sent)")
        return
    pressed = run("press", "escape")
    if not pressed["ok"]:
        say(f"     press escape: {outcome(pressed)}")
    for _ in range(5):
        if center_open() is False:
            say("PASS notification center closed")
            return
        time.sleep(1)
    say("NOTE notification center left open")


def main():
    say(f"bench nonce {NONCE}")
    say("ACTION open Notification Center by clicking the date and time in the menu bar; waiting up to 120 s")
    deadline = time.monotonic() + 120
    while center_open() is not True:
        if time.monotonic() > deadline:
            say("INCONCLUSIVE Notification Center was not opened")
            return 3
        time.sleep(2)
    baseline, result = bench_view()
    if baseline is None:
        say(f"INCONCLUSIVE list-notifications failed before posting: {outcome(result)}")
        return 3
    say(f"     before: {len(baseline['foreign'])} other notifications present (keys hash {keys_hash(baseline['foreign'])})")
    try:
        scenario()
    except Halt:
        say("STOP no further scenario mutations; cleaning up bench notifications only")
    except subprocess.CalledProcessError:
        say("INCONCLUSIVE osascript could not post a notification")
        INCONCLUSIVE.append("post")
    finally:
        left = cleanup()
    if left is not None:
        check(not left["ours"], f"cleanup removed every bench notification ({len(left['ours'])} left)")
        before, after = set(baseline["foreign"]), set(left["foreign"])
        missing, added = before - after, after - before
        say(f"     after: {len(after)} other notifications present (keys hash {keys_hash(after)})")
        check(not missing, f"{len(before)} other notifications untouched: {len(missing)} missing, {len(added)} arrived during the run")
    else:
        FAILS.append("cleanup could not finish")
    close_center()
    if FAILS:
        say(f"RESULT FAIL ({len(FAILS)} failed checks)")
        return 1
    if INCONCLUSIVE:
        say(f"RESULT INCONCLUSIVE ({', '.join(INCONCLUSIVE)})")
        return 3
    say("RESULT PASS")
    return 0


def guarded():
    home = tempfile.mkdtemp(prefix="adbench-34-home-", dir="/tmp")
    ENV["AGENT_DESKTOP_HOME"] = home
    code = 1
    try:
        code = main()
    except BaseException as error:
        say(f"ERROR {type(error).__name__}")
        code = 1
        try:
            STATE["cleaning"] = True
            cleanup()
            close_center()
        except BaseException as again:
            say(f"ERROR {type(again).__name__}")
    finally:
        shutil.rmtree(home, ignore_errors=True)
        say(f"UNDO removed agent-desktop state dir {home}")
    return code


if __name__ == "__main__":
    sys.exit(guarded())
ADBENCH_PY_EOF
cd "$R" && git fetch -q origin && git worktree prune && rm -rf "$W" && git worktree add -q --detach "$W" "$SHA" || exit 1
while pgrep -x cargo >/dev/null; do sleep 5; done
(cd "$W" && CARGO_BUILD_JOBS=2 cargo build --release -p agent-desktop) || exit 1
if pgrep -x Calculator >/dev/null; then echo "NOTE Calculator already running; desktop-space reset skipped (it would quit Calculator)"; else git -C "$R" show origin/chore/speed-1.4-tasks:bench-tools/desktop_space.sh > "$SP" && bash "$SP"; echo "desktop-space reset ran (exit $?)"; fi
AD="$W/target/release/agent-desktop" python3 -I "$PY"; echo "exit $?"
```

## #88 LIFECYCLE-2 + LIFECYCLE-12: macOS launch watches the launched process; several restored windows are not an error

Head: `46f397a712f72d8f01b580c5b4054d1961c0e043` (unchanged). Revision 2 (2026-11-10) after the not-run verdict:
- Fixtures and state live under `$HOME/.agent-desktop-bench-target/adl-check-<timestamp>`, nothing in /tmp.
- The applets get `plutil -replace CFBundleIdentifier`. All four apps are `codesign -f -s -` signed, then `lsregister -f`.
- A Swift `resolve` helper calls `NSWorkspace.shared.urlForApplication(withBundleIdentifier:)`, the product's call at
  `launch_bridge.m:142-143`, for every id. If any is nil it prints `fixture unresolvable: <id>` and HALTs before any case.
- The build runs before the lock, through the bench's `bench_build` (`perf-speed-cost/bench/cargo_env.sh`).
  `origin/chore/speed-1.4-tasks` has no `bench/`, so `bench/serial_cargo.sh` was not usable.
- The cases run as `$ADL_DESKTOP_LOCK bash -c cases`. **The Mac worker sets `ADL_DESKTOP_LOCK` to the bench's
  desktop-lock command prefix**; the script HALTs if it is unset.
- `desktop_space.sh` is copied with sed: its `mktemp -d` points at the check's own dir and its trailing `rm -rf` is
  removed.
- Window counts come from a Swift CGWindowList helper: on-screen, layer 0, pids of that bundle id. No System Events.
- No `rm -rf` anywhere. The EXIT trap kills the fixtures, runs `lsregister -u` on each, prints `UNREGISTER ran for:
  ...`, and prints the dir left for George with the exact `git worktree remove` command.
- Laptop lead's patch: Calculator is quit only if the check launched it. If Calculator was already running, the
  desktop-space reset is skipped and Calculator stays running, each with a NOTE line.
- `launch` cannot take an app path: macOS `launch` refuses an identifier containing `/` with INVALID_ARGS
  (`validate_app_identifier`, `crates/macos/src/system/launch.rs:275-285`). It takes a display name or a bundle id only.

Expected: 6 PASS (every case prints its full envelope first). Before the fix, the LIFECYCLE-2 cases answer `ok: true`
with a dead pid and the LIFECYCLE-12 cases answer `AMBIGUOUS_TARGET`. INVALID means the precondition was not met: 2
on-screen windows, none focused.

Run notes from the Mac worker: the harness has no ready `ADL_DESKTOP_LOCK` prefix, so the worker wrapped `lockq.sh` take/release itself. The fetch of `chore/speed-1.4-tasks` inside the script is a network dependency that halted one attempt; future scripts copy desktop_space.sh before building, or retry the fetch.

Calculator observation from an earlier run, cause unconfirmed, on main and not part of #88: `narrow_to_real_windows`
(`crates/macos/src/system/window/window_inventory.rs`) keeps a window whose `minimized` is known even when it is
`visible: false`. So launch can report a window that is not yet on screen, with zero bounds; an early return can
report none. Ledgered as LAUNCH-WIN-1.

```bash
cat > "$HOME/.agent-desktop-bench-target/adl-check.sh" <<'ADLEOF'
#!/bin/bash
# Mac check for PR #88 (LIFECYCLE-2 + LIFECYCLE-12) at a fixed head. Model-free: each case prints its
# full envelope, then PASS, FAIL or INVALID (fixture precondition not met). No System Events, no UI
# scripting, no Finder or TextEdit. Fixtures live under $HOME/.agent-desktop-bench-target (bench-owned).
# Run:  ADL_DESKTOP_LOCK='<the bench desktop-lock wrapper, a command prefix>' bash adl-check.sh
set -u
SHA=46f397a712f72d8f01b580c5b4054d1961c0e043
R=$HOME/Projects/agent-desktop
BENCH=$HOME/.t3/worktrees/agent-desktop/perf-speed-cost/bench
W=$HOME/.agent-desktop-bench-target/adl-check-$(date +%Y%m%d-%H%M%S)
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
APPS="AdlQuit AdlTwo AdlAppletQuit AdlJxaTwo"
halt() { echo "HALT: $*"; exit 2; }
[ -n "${ADL_DESKTOP_LOCK:-}" ] || halt "set ADL_DESKTOP_LOCK to the command prefix the bench uses to take the desktop lock"
[ -f "$BENCH/cargo_env.sh" ] || halt "missing $BENCH/cargo_env.sh (bench_build)"
mkdir -p "$W" || halt "cannot create $W"
cleanup() {
  for a in $APPS; do pkill -f "$W/$a.app" 2>/dev/null; [ -d "$W/$a.app" ] && "$LSREG" -u "$W/$a.app"; done
  echo "UNREGISTER ran for: $APPS"
  echo "left for George: $W (worktree $W/src: git -C $R worktree remove --force $W/src)"
}
trap cleanup EXIT

# 1. Build outside the desktop lock, through the bench's own serialized build.
git -C "$R" fetch -q origin && git -C "$R" worktree add -q --detach "$W/src" "$SHA" || halt "worktree"
( cd "$BENCH/.." && source bench/cargo_env.sh && bench_build "$W/src" ) || halt "bench_build failed"
AD="$W/src/target/release/agent-desktop"; [ -x "$AD" ] || halt "no binary at $AD"
export AGENT_DESKTOP_HOME="$W/home"; mkdir -p "$AGENT_DESKTOP_HOME"; chmod 700 "$AGENT_DESKTOP_HOME"

# 2. Fixtures: two AppKit Swift apps, an AppleScript applet, a JXA stay-open applet; each gets a bundle id,
#    an ad-hoc signature and a Launch Services registration. Two Swift helpers: id resolution and windows.
plist() { # app-dir name bundle-id
  printf '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>%s</string><key>CFBundleExecutable</key><string>%s</string><key>CFBundleName</key><string>%s</string><key>CFBundlePackageType</key><string>APPL</string><key>NSPrincipalClass</key><string>NSApplication</string></dict></plist>\n' "$3" "$2" "$2" > "$1/Contents/Info.plist"
}
swift_app() { # name bundle-id source
  mkdir -p "$W/$1.app/Contents/MacOS"; printf '%s\n' "$3" > "$W/$1.swift"
  swiftc -O -o "$W/$1.app/Contents/MacOS/$1" "$W/$1.swift" -framework AppKit || halt "swiftc $1"
  plist "$W/$1.app" "$1" "$2"
}
swift_app AdlQuit dev.adl.check.quit 'import AppKit
final class D: NSObject, NSApplicationDelegate { func applicationDidFinishLaunching(_ n: Notification) { DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { exit(0) } } }
let a = NSApplication.shared; let d = D(); a.delegate = d; a.setActivationPolicy(.regular); a.run()'
swift_app AdlTwo dev.adl.check.two 'import AppKit
final class D: NSObject, NSApplicationDelegate { var w: [NSWindow] = []
 func applicationDidFinishLaunching(_ n: Notification) { for i in 0..<2 { let x = NSWindow(contentRect: NSRect(x: 120 + i * 320, y: 300, width: 280, height: 160), styleMask: [.titled], backing: .buffered, defer: false); x.title = "adl-two-\(i)"; x.orderFrontRegardless(); w.append(x) } } }
let a = NSApplication.shared; let d = D(); a.delegate = d; a.setActivationPolicy(.regular); a.run()'
osacompile -o "$W/AdlAppletQuit.app" -e 'delay 0.5' || halt "osacompile applet"
printf '%s\n' "ObjC.import('Cocoa');" "var wins = [];" \
 "function run() { for (var i = 0; i < 2; i++) { var x = \$.NSWindow.alloc.initWithContentRectStyleMaskBackingDefer(\$.NSMakeRect(120 + 320 * i, 520, 280, 160), \$.NSWindowStyleMaskTitled, \$.NSBackingStoreBuffered, false); x.title = 'adl-jxa-' + i; x.orderFrontRegardless; wins.push(x); } }" \
 "function idle() { return 30; }" > "$W/two.js"
osacompile -l JavaScript -s -o "$W/AdlJxaTwo.app" "$W/two.js" || halt "osacompile jxa"
plutil -replace CFBundleIdentifier -string dev.adl.check.appletquit "$W/AdlAppletQuit.app/Contents/Info.plist"
plutil -replace CFBundleIdentifier -string dev.adl.check.jxatwo "$W/AdlJxaTwo.app/Contents/Info.plist"
for a in $APPS; do codesign -f -s - "$W/$a.app" 2>/dev/null || halt "codesign $a"; "$LSREG" -f "$W/$a.app"; done
printf '%s\n' 'import AppKit' 'var bad = false' \
 'for id in CommandLine.arguments.dropFirst() { if NSWorkspace.shared.urlForApplication(withBundleIdentifier: id) == nil { print("fixture unresolvable: \(id)"); bad = true } else { print("fixture resolves: \(id)") } }' \
 'exit(bad ? 1 : 0)' > "$W/resolve.swift"
swiftc -O -o "$W/resolve" "$W/resolve.swift" -framework AppKit || halt "swiftc resolve"
printf '%s\n' 'import AppKit' 'let id = CommandLine.arguments[1]' \
 'let pids = Set(NSRunningApplication.runningApplications(withBundleIdentifier: id).map { Int($0.processIdentifier) })' \
 'let list = (CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]]) ?? []' \
 'print(list.filter { pids.contains(($0[kCGWindowOwnerPID as String] as? Int) ?? -1) && (($0[kCGWindowLayer as String] as? Int) ?? 1) == 0 }.count)' > "$W/winlist.swift"
swiftc -O -o "$W/winlist" "$W/winlist.swift" -framework AppKit || halt "swiftc winlist"
"$W/resolve" dev.adl.check.quit dev.adl.check.two dev.adl.check.appletquit dev.adl.check.jxatwo || halt "a fixture does not resolve through NSWorkspace; no case was run"

# 3. Desktop Space prelude: the bench script, copied, with its mktemp pointed at this check's dir and
#    its trailing rm -rf removed (the dir it makes is left under $W).
git -C "$R" fetch -q origin chore/speed-1.4-tasks || halt "fetch chore/speed-1.4-tasks"
git -C "$R" show origin/chore/speed-1.4-tasks:bench-tools/desktop_space.sh \
 | sed -e "s#mktemp -d [^)]*#mktemp -d $W/space.XXXX#" -e '/^rm -rf /d' > "$W/desktop_space.sh"

field() { python3 -c 'import json,sys; e=json.load(sys.stdin); v=e
for k in sys.argv[1].split("."):
    v = v.get(k) if isinstance(v, dict) else None
print("" if v is None else v)' "$1"; }
quit_case() { # label bundle-id extra-flag
  local t0 t1 out code pid secs
  t0=$(date +%s); out=$("$AD" launch "$2" $3 --timeout 20000); t1=$(date +%s); secs=$(( t1 - t0 ))
  code=$(printf '%s' "$out" | field error.code); pid=$(printf '%s' "$out" | field error.details.pid)
  echo "== LIFECYCLE-2 $1 ($secs s): $out"
  if [ "$code" = APP_UNRESPONSIVE ] && [ "$secs" -lt 10 ] && [ -n "$pid" ] && ! kill -0 "$pid" 2>/dev/null; then echo "PASS $1"
  else echo "FAIL $1 (expected APP_UNRESPONSIVE in under 10 s with an exited details.pid; before the fix: ok:true with a dead pid)"; fi
}
two_case() { # label bundle-id app-name
  local out ok win sug n focused
  out=$("$AD" launch "$2" --timeout 20000)
  n=0; for _ in $(seq 1 20); do n=$("$W/winlist" "$2"); [ "$n" = 2 ] && break; sleep 0.5; done
  echo "list-windows --app $3: $("$AD" list-windows --app "$3")"
  focused=$("$AD" list-windows --app "$3" | grep -o '"is_focused":true' | wc -l | tr -d ' ')
  ok=$(printf '%s' "$out" | field ok); win=$(printf '%s' "$out" | field data.window.id); sug=$(printf '%s' "$out" | field data.suggestion)
  echo "== LIFECYCLE-12 $1 (on-screen windows: $n, focused: $focused): $out"
  if [ "$n" != 2 ] || [ "$focused" != 0 ]; then echo "INVALID $1 (precondition: 2 on-screen windows, none focused)"
  elif [ "$ok" = True ] && [ -z "$win" ] && [ "$(printf '%s' "$sug" | grep -o 'w-[0-9]*' | sort -u | wc -l | tr -d ' ')" = 2 ]; then echo "PASS $1"
  else echo "FAIL $1 (expected ok:true, no data.window, data.suggestion naming 2 window ids; before the fix: AMBIGUOUS_TARGET)"; fi
  pkill -f "$W/$3.app" 2>/dev/null
}
control_case() {
  local out
  out=$("$AD" launch Calculator --timeout 20000); echo "== control Calculator: $out"
  if [ -n "$(printf '%s' "$out" | field data.window.id)" ]; then echo "PASS control-calculator"
  else echo "FAIL control-calculator (a one-window app must still report its window)"; fi
  echo "list-windows --app Calculator: $("$AD" list-windows --app Calculator)"
  if [ "$CALC_WAS_RUNNING" = 0 ]; then "$AD" close-app Calculator >/dev/null 2>&1; echo "Calculator quit (this check launched it)"
  else echo "NOTE Calculator left running (it was running before the check)"; fi
}
cases() {
  CALC_WAS_RUNNING=0; pgrep -x Calculator >/dev/null && CALC_WAS_RUNNING=1
  if [ "$CALC_WAS_RUNNING" = 1 ]; then echo "NOTE Calculator already running: desktop-space reset skipped (it quits Calculator)"
  else bash "$W/desktop_space.sh"; fi
  quit_case swift-activate dev.adl.check.quit --activate
  quit_case swift-background dev.adl.check.quit ""
  quit_case applet-activate dev.adl.check.appletquit --activate
  two_case swift-two dev.adl.check.two AdlTwo
  two_case jxa-two dev.adl.check.jxatwo AdlJxaTwo
  control_case
}
export -f cases quit_case two_case control_case field
export W AD
# 4. The cases run inside the desktop lock, after the build.
$ADL_DESKTOP_LOCK bash -c cases
ADLEOF
ADL_DESKTOP_LOCK='<bench desktop-lock command prefix>' bash "$HOME/.agent-desktop-bench-target/adl-check.sh"
```

## #58 probe: ACTION-12 on a real Mac, timestamped (diagnostic, not a pass/fail check)

Builds branch `probe/pointer-focus-once` at `8959fc5c91f647e791a04863fec24de59399e9b9`: #58's `d57dadfa` plus logging only
(8 files, +209/-15). #58's branch is untouched. Send back `/tmp/pfo/probe.log` (one JSON object per line).

What the block does:
- Fetches the branch into `~/Projects/agent-desktop` (no `git switch`) and builds a detached worktree at `/tmp/pfo/src`,
  after any running cargo finishes (`CARGO_BUILD_JOBS=4`).
- Builds the e2e fixture into `/tmp/pfo/fixture` and two Swift helpers:
  - the floating-panel overlay;
  - `pfo-windows`, which reports the NSWorkspace front pid and the CGWindowList z-order (top 12: owner pid and layer;
    bounds only for the fixture, Calculator and overlay pids; never titles).
- Uses no osascript, no System Events, no lsappinfo, no Finder and no TextEdit.
- Runs its own desktop prelude with the probe binary: `launch Calculator --activate`, then quits Calculator only if the
  probe launched it.
- Opens the fixture and waits for its window before every scenario. Bounds come from `get <ref> --property bounds`.
- `trap cleanup EXIT` stops the overlay, quits the fixture, quits Calculator only if the probe launched it (otherwise it
  says Calculator stays running) and removes the worktree, printing one confirmation line for each.

Logging uses the existing `tracing` channel, target `pfo` (`RUST_LOG=pfo=debug`). Each event carries a monotonic `t_us`,
`wall_us` and `pid`. Events:
- `lease.acquire`;
- `point.resolve` and `point.observe`;
- `front.before_raise` (with its classification), `raise.resolve_window`, `raise.call`, `raise.result` (the AXRaise code)
  and `front.after_raise`;
- `hit_test.single` and `hit_test.candidate` (reaches_target, unknown, intercepted_by or error);
- `focus.outcome`;
- `attempt.end`;
- `loop.branch` (success, stop_front_changed, retry, front_changed_but_settling, return_not_retryable or timeout).

With `PFO_POLL_FRONT=1` (CP1 only), the front window is also polled every 20 ms for up to 1 s after each raise, logging
`front.poll_became_target` or `front.poll_never_target`. CP1's timing is therefore not representative.

Scenarios, each run once:

| Scenario | Front app | Target | Budget |
|---|---|---|---|
| B1 | Calculator | uncovered hover | 3 s |
| overlay on | | z-order logged | |
| D1 | fixture | covered hover | 3 s |
| C1, C2 | Calculator | covered hover | 5 s |
| CP1 | Calculator | as C, with the front poll | 5 s |
| E1 | Calculator | drag from context-target onto the covered hover-target | 5 s |

It takes about 1.5 min after the build. The laptop lead added the `pgrep -x cargo` wait. `bash -n` and py_compile are
clean on the laptop; the Python was dry-run against stubs and has not run on a Mac.

The agent's reading of the failed check, before the log:
- D1's error comes only from `crates/core/src/commands/pointer_action.rs:128-132`, the deadline check after
  `require_receives_events` passed. So on the real Mac the hit test returned ReachesTarget or Unknown under the floating
  panel, not occluded, which is why "occluded" never appears.
- A likely cause: the macOS hit test retries a hit outside the target's chain against the target's own application
  (`crates/macos/src/tree/hit_test.rs`, `needs_application_retry` / `hit_test_in_application`), which does not see
  another process's panel. The `hit_test.*` events settle it.

```bash
#!/usr/bin/env bash
# #58 probe. Mac only, model-free. Builds probe/pointer-focus-once at 8959fc5c in a
# detached worktree, runs the covered-target scenarios, writes /tmp/pfo/probe.log.
set -u
SHA=8959fc5c91f647e791a04863fec24de59399e9b9
REPO="$HOME/Projects/agent-desktop"
B=/tmp/pfo/src/target/release/agent-desktop
if pgrep -x AgentDeskFixture >/dev/null; then echo "STOP: AgentDeskFixture is already running"; exit 1; fi
calc_was_running=0
pgrep -x Calculator >/dev/null && calc_was_running=1
[ "$calc_was_running" = 1 ] && echo "note: Calculator was already running; the probe uses it and leaves it running"
git -C "$REPO" worktree remove --force /tmp/pfo/src 2>/dev/null
rm -rf /tmp/pfo && mkdir -p /tmp/pfo/fixture
cleanup() {
    if [ -f /tmp/pfo/overlay.pid ]; then kill "$(cat /tmp/pfo/overlay.pid)" 2>/dev/null; rm -f /tmp/pfo/overlay.pid; fi
    pgrep -f /tmp/pfo/overlay >/dev/null && echo "cleanup: WARNING overlay still running" || echo "cleanup: overlay not running"
    [ -x "$B" ] && "$B" close-app AgentDeskFixture --force >/dev/null 2>&1
    pgrep -x AgentDeskFixture >/dev/null && echo "cleanup: WARNING AgentDeskFixture still running" || echo "cleanup: AgentDeskFixture not running"
    if [ "$calc_was_running" = 0 ]; then
        [ -x "$B" ] && "$B" close-app Calculator >/dev/null 2>&1
        pgrep -x Calculator >/dev/null && echo "cleanup: WARNING Calculator still running" || echo "cleanup: Calculator quit (the probe launched it)"
    else
        echo "cleanup: Calculator was running before the probe and stays running"
    fi
    git -C "$REPO" worktree remove --force /tmp/pfo/src 2>/dev/null
    [ -e /tmp/pfo/src ] && echo "cleanup: WARNING worktree /tmp/pfo/src still present" || echo "cleanup: worktree /tmp/pfo/src removed"
    echo "cleanup: log kept at /tmp/pfo/probe.log ($(wc -l < /tmp/pfo/probe.log 2>/dev/null || echo 0) lines)"
}
trap cleanup EXIT
git -C "$REPO" fetch origin probe/pointer-focus-once || exit 1
git -C "$REPO" worktree add --force --detach /tmp/pfo/src "$SHA" || exit 1
[ "$(git -C /tmp/pfo/src rev-parse HEAD)" = "$SHA" ] || { echo "STOP: worktree is not at $SHA"; exit 1; }
while pgrep -x cargo >/dev/null; do sleep 5; done
(cd /tmp/pfo/src && CARGO_BUILD_JOBS=4 cargo build --release -p agent-desktop) || exit 1
bash /tmp/pfo/src/tests/fixture-app/build.sh /tmp/pfo/fixture >/dev/null || exit 1
cat > /tmp/pfo/overlay.swift <<'PFO_EOF'
import AppKit

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let numbers = CommandLine.arguments.dropFirst().compactMap { Double($0) }
guard numbers.count == 4, let primary = NSScreen.screens.first else { exit(2) }
let frame = NSRect(x: numbers[0], y: primary.frame.height - numbers[1] - numbers[3],
                   width: numbers[2], height: numbers[3])
let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel],
                    backing: .buffered, defer: false)
panel.level = .floating
panel.hidesOnDeactivate = false
panel.backgroundColor = .systemRed
let cover = NSButton(title: "pfo-cover", target: nil, action: nil)
cover.frame = NSRect(origin: .zero, size: frame.size)
panel.contentView?.addSubview(cover)
panel.orderFrontRegardless()
application.run()
PFO_EOF
cat > /tmp/pfo/windows.swift <<'PFO_EOF'
import AppKit

let detailed = Set(CommandLine.arguments.dropFirst().compactMap { Int32($0) })
let front = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let list = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
var windows: [[String: Any]] = []
for window in list.prefix(12) {
    let pid = (window[kCGWindowOwnerPID as String] as? Int32) ?? -1
    var entry: [String: Any] = ["pid": pid, "layer": window[kCGWindowLayer as String] ?? -1]
    if detailed.contains(pid) {
        entry["window_id"] = window[kCGWindowNumber as String] ?? -1
        entry["bounds"] = window[kCGWindowBounds as String] ?? [:]
    }
    windows.append(entry)
}
let report: [String: Any] = ["front_pid": front, "z_order_front_to_back": windows]
if let data = try? JSONSerialization.data(withJSONObject: report),
   let text = String(data: data, encoding: .utf8) {
    print(text)
}
PFO_EOF
cat > /tmp/pfo/probe.py <<'PFO_EOF'
import json, os, subprocess, sys, time

ROOT = os.environ.get("PFO_ROOT", "/tmp/pfo")
BIN = ROOT + "/src/target/release/agent-desktop"
APP = "AgentDeskFixture"
OTHER = "Calculator"
LOG = open(ROOT + "/probe.log", "a")
T0 = time.monotonic()
overlay = None


def log(event, **fields):
    line = json.dumps({"src": "probe", "event": event, "t_ms": round((time.monotonic() - T0) * 1000, 1),
                       "wall_us": int(time.time() * 1e6), **fields})
    print(line, flush=True)
    LOG.write(line + "\n")
    LOG.flush()


def pid_of(name):
    out = subprocess.run(["pgrep", "-x", name], capture_output=True, text=True).stdout.split()
    return int(out[0]) if len(out) == 1 else None


def windows():
    pids = [str(pid) for pid in (pid_of(APP), pid_of(OTHER), overlay.pid if overlay else None) if pid]
    out = subprocess.run([ROOT + "/pfo-windows", *pids], capture_output=True, text=True).stdout
    try:
        return json.loads(out)
    except ValueError:
        return {"unparsed": out}


def ad(*args, traced=False, poll_front=False):
    env = dict(os.environ)
    if traced:
        env["RUST_LOG"] = "pfo=debug"
    if poll_front:
        env["PFO_POLL_FRONT"] = "1"
    started = time.monotonic()
    proc = subprocess.run([BIN, *args], capture_output=True, text=True, timeout=90, env=env)
    elapsed = round((time.monotonic() - started) * 1000, 1)
    try:
        body = json.loads(proc.stdout)
    except ValueError:
        body = {"unparsed_stdout": proc.stdout}
    events = []
    for line in proc.stderr.splitlines():
        marker = line.find(" pfo: ")
        if marker < 0:
            continue
        try:
            events.append(json.loads(line[marker + 6:]))
        except ValueError:
            events.append({"unparsed": line})
    return body, elapsed, events


def bring_front(name):
    if name == APP:
        ad("focus-window", "--app", APP)
    else:
        subprocess.run(["open", "-a", name])
    target = None
    for _ in range(50):
        target = pid_of(name)
        state = windows()
        if target and state.get("front_pid") == target:
            time.sleep(0.3)
            log("front_reached", app=name, pid=target, windows=windows())
            return True
        time.sleep(0.1)
    log("front_not_reached", app=name, pid=target, windows=windows())
    return False


def wait_fixture_window():
    for _ in range(60):
        body, _, _ = ad("list-windows", "--app", APP)
        if body.get("ok") is True and body.get("data"):
            log("fixture_window_present", count=len(body["data"]))
            return True
        time.sleep(0.5)
    log("fixture_window_missing", last=body)
    return False


def find_ref(name):
    body = {}
    for _ in range(100):
        body, _, _ = ad("find", "--app", APP, "--role", "button", "--name", name, "--exact", "--first")
        match = (body.get("data") or {}).get("match") or {}
        if body.get("ok") is True and match.get("ref_id"):
            return match["ref_id"]
        time.sleep(0.2)
    log("setup_fail", reason=f"{name} not found", last=body)
    sys.exit(1)


def bounds_of(ref):
    body, _, _ = ad("get", ref, "--property", "bounds")
    return (body.get("data") or {}).get("value"), body


def scenario(label, front, command, poll_front=False):
    if not wait_fixture_window():
        return
    reached = bring_front(front)
    log("scenario_start", label=label, front=front, front_reached=reached, command=command, poll_front=poll_front)
    body, elapsed, events = ad("--headed", *command, traced=True, poll_front=poll_front)
    for event in events:
        line = json.dumps({"src": "binary", "label": label, **event})
        print(line, flush=True)
        LOG.write(line + "\n")
    LOG.flush()
    log("scenario_end", label=label, elapsed_ms=elapsed, ok=body.get("ok"), error=body.get("error"),
        data=body.get("data"), windows_after=windows())


def main():
    global overlay
    log("start", bin=BIN, fixture_pid=pid_of(APP), calculator_pid=pid_of(OTHER))
    if not wait_fixture_window():
        sys.exit(1)
    ad("focus-window", "--app", APP)
    ad("move-window", "--app", APP, "--x", "40", "--y", "60")
    ad("resize-window", "--app", APP, "--width", "1000", "--height", "700")
    hover = find_ref("hover-target")
    for _ in range(3):
        body, _, _ = ad("scroll-to", hover)
        if body.get("ok") is True:
            break
        time.sleep(0.5)
    time.sleep(0.5)
    hover = find_ref("hover-target")
    context = find_ref("context-target")
    box, raw = bounds_of(hover)
    cbox, craw = bounds_of(context)
    log("targets", hover=hover, hover_bounds=box, context=context, context_bounds=cbox)
    if not box or not cbox:
        log("setup_fail", reason="bounds unavailable", hover_get=raw, context_get=craw)
        sys.exit(1)
    cover = [box["x"] - 6, box["y"] - 6, box["width"] + 12, box["height"] + 12]

    scenario("B1 calculator-front uncovered hover", OTHER, ["hover", hover, "--timeout-ms", "3000"])

    overlay = subprocess.Popen([ROOT + "/overlay", *[str(value) for value in cover]])
    open(ROOT + "/overlay.pid", "w").write(str(overlay.pid))
    time.sleep(1.5)
    log("overlay_started", pid=overlay.pid, running=overlay.poll() is None, cover=cover, windows=windows())

    scenario("D1 fixture-front covered hover", APP, ["hover", hover, "--timeout-ms", "3000"])
    scenario("C1 calculator-front covered hover", OTHER, ["hover", hover, "--timeout-ms", "5000"])
    scenario("C2 calculator-front covered hover", OTHER, ["hover", hover, "--timeout-ms", "5000"])
    scenario("CP1 calculator-front covered hover, front polled 1 s after each raise", OTHER,
             ["hover", hover, "--timeout-ms", "5000"], poll_front=True)
    scenario("E1 calculator-front drag onto covered drop", OTHER,
             ["drag", "--from", context, "--to", hover, "--timeout-ms", "5000"])

    overlay.kill()
    overlay.wait()
    os.remove(ROOT + "/overlay.pid")
    log("overlay_stopped", pid=overlay.pid)
    log("done")


main()
PFO_EOF
swiftc -O -o /tmp/pfo/overlay /tmp/pfo/overlay.swift -framework AppKit || exit 1
swiftc -O -o /tmp/pfo/pfo-windows /tmp/pfo/windows.swift -framework AppKit || exit 1
"$B" launch Calculator --activate >/dev/null 2>&1
sleep 1
[ "$calc_was_running" = 0 ] && "$B" close-app Calculator >/dev/null 2>&1
sleep 1
open /tmp/pfo/fixture/AgentDeskFixture.app
python3 /tmp/pfo/probe.py
echo "probe exit $? ; send /tmp/pfo/probe.log"
```

## #58 (b) hit-test probe: why the macOS hit test answers `unknown` (diagnostic, not pass/fail)

Builds `probe/hit-test-unknown` at `7de03c996715de413e4dcfba7eade3ffc56f37d6`. That is the #58 probe (`8959fc5c`, core
`pfo` events) plus `pfo` logging at every `unknown` exit of `crates/macos/src/tree/hit_test.rs`. The branch never
merges; #58 and every fix branch are untouched. Compile check: CI run 42745854796 green on all 13 jobs at that SHA, the
macOS Test job included. On this throwaway branch only, `hit_test.rs` and `pointer_action.rs` carry an `//! @generated`
marker so the 400-line rule let CI reach the compile.

Purpose: find out why the hit test answers `unknown` on every call, covered or not, and whether the AX primary read
can be made to answer. Control's condition: fix a broken primary read first; the window list is only the fallback.
There is no pass/fail; send the log back.

Events logged:
- one `hit_test.unknown_exit` per `unknown` exit, from `bounds_none`, `zero_size`, `clipping_incomplete`,
  `system_wide_null`, `system_wide_no_element_at_position`, `system_wide_ancestry_incomplete`,
  `application_retry_pid_error`, `application_retry_no_element_at_position`, `application_retry_classification=<variant>`,
  `classification_ancestor_of_target` and `intercepted_by_error`;
- `hit_test.element_at_position` (ax_error, hit_null and the point) for every system-wide and per-app
  AXUIElementCopyElementAtPosition;
- `hit_test.system_wide_classification`.

The rig:
- the fixture, Calculator, and the floating-panel overlay over the hover target;
- everything in a fresh `mktemp -d /tmp/pfo-hit.XXXXXX`, which is left in place;
- a detached worktree at the SHA, a wait for other cargo runs (30 min cap), then `CARGO_BUILD_JOBS=4 cargo build
  --release`;
- the desktop prelude (Calculator quit only if the probe launched it) and a fixture-window wait before each step;
- a `trap cleanup EXIT` that stops the overlay, quits the fixture, quits Calculator only if the probe launched it and
  removes the worktree, printing one line for each.

It uses no `rm` (it deletes only its own `$R/overlay.pid`), no osascript, no System Events, no lsappinfo, and no
window titles of other processes.

The helpers:
- `pfo-ax X Y N PID` makes N calls 100 ms apart. Each runs system-wide CopyElementAtPosition (AX error, hit pid,
  AXRole, time), system-wide AXFocusedApplication (AX error, pid, time) and the fixture's per-app
  CopyElementAtPosition. It also prints AXIsProcessTrusted.
- `pfo-windows` reports the front pid and the CG z-order (pid and layer; bounds only for the test pids).

Steps:
1. U, uncovered: with Calculator in front, `pfo-ax` 5x before the raise, then `focus-window --app AgentDeskFixture`,
   0.3 s, and `pfo-ax` 5x after it.
2. U1 and U2: a traced `--headed hover` with the fixture in front, then with Calculator in front.
3. Overlay on, with the z-order logged.
4. V, covered: the same raise cycle, then traced hovers V1 and V2.

Questions it answers:
- which exit returns `unknown`, covered and uncovered;
- whether the system-wide calls also fail from a plain helper process, and whether only before the raise;
- whether the per-app element answers where the system-wide one does not;
- what the AX hit returns under the overlay.

Laptop checks: `bash -n` and py_compile are clean, the Python dry-ran against stubs, and grep finds no rm, osascript,
System Events or lsappinfo. The Swift helpers have not been compiled; the Mac's swiftc is the first compile.

```bash
#!/usr/bin/env bash
# #58/(b) hit-test probe. Mac only, model-free. Builds probe/hit-test-unknown at 7de03c99
# in a detached worktree under a fresh mktemp directory and writes $R/probe.log.
set -u
SHA=7de03c996715de413e4dcfba7eade3ffc56f37d6
REPO="$HOME/Projects/agent-desktop"
if pgrep -x AgentDeskFixture >/dev/null; then echo "STOP: AgentDeskFixture is already running"; exit 1; fi
calc_was_running=0
pgrep -x Calculator >/dev/null && calc_was_running=1
[ "$calc_was_running" = 1 ] && echo "note: Calculator was already running; the probe uses it and leaves it running"
R=$(mktemp -d /tmp/pfo-hit.XXXXXX) || exit 1
export PFO_ROOT="$R"
B="$R/src/target/release/agent-desktop"
mkdir -p "$R/fixture"
echo "probe directory: $R"
cleanup() {
    if [ -f "$R/overlay.pid" ]; then kill "$(cat "$R/overlay.pid")" 2>/dev/null; fi
    pgrep -f "$R/overlay" >/dev/null && echo "cleanup: WARNING overlay still running" || echo "cleanup: overlay not running"
    [ -x "$B" ] && "$B" close-app AgentDeskFixture --force >/dev/null 2>&1
    pgrep -x AgentDeskFixture >/dev/null && echo "cleanup: WARNING AgentDeskFixture still running" || echo "cleanup: AgentDeskFixture not running"
    if [ "$calc_was_running" = 0 ]; then
        [ -x "$B" ] && "$B" close-app Calculator >/dev/null 2>&1
        pgrep -x Calculator >/dev/null && echo "cleanup: WARNING Calculator still running" || echo "cleanup: Calculator quit (the probe launched it)"
    else
        echo "cleanup: Calculator was running before the probe and stays running"
    fi
    git -C "$REPO" worktree remove --force "$R/src" 2>/dev/null
    [ -e "$R/src" ] && echo "cleanup: WARNING worktree $R/src still present" || echo "cleanup: worktree $R/src removed"
    echo "cleanup: directory $R left in place; log $R/probe.log ($(wc -l < "$R/probe.log" 2>/dev/null || echo 0) lines)"
}
trap cleanup EXIT
git -C "$REPO" fetch origin probe/hit-test-unknown || exit 1
git -C "$REPO" worktree add --detach "$R/src" "$SHA" || exit 1
[ "$(git -C "$R/src" rev-parse HEAD)" = "$SHA" ] || { echo "STOP: worktree is not at $SHA"; exit 1; }
waited=0
while pgrep -x cargo >/dev/null; do
    [ "$waited" -ge 1800 ] && { echo "STOP: another cargo still running after 30 min"; exit 1; }
    echo "waiting for another cargo to finish ($waited s)"; sleep 15; waited=$((waited + 15))
done
(cd "$R/src" && CARGO_BUILD_JOBS=4 cargo build --release -p agent-desktop) || exit 1
bash "$R/src/tests/fixture-app/build.sh" "$R/fixture" >/dev/null || exit 1
cat > "$R/overlay.swift" <<'PFO_EOF'
import AppKit

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let numbers = CommandLine.arguments.dropFirst().compactMap { Double($0) }
guard numbers.count == 4, let primary = NSScreen.screens.first else { exit(2) }
let frame = NSRect(x: numbers[0], y: primary.frame.height - numbers[1] - numbers[3],
                   width: numbers[2], height: numbers[3])
let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel],
                    backing: .buffered, defer: false)
panel.level = .floating
panel.hidesOnDeactivate = false
panel.backgroundColor = .systemRed
let cover = NSButton(title: "pfo-cover", target: nil, action: nil)
cover.frame = NSRect(origin: .zero, size: frame.size)
panel.contentView?.addSubview(cover)
panel.orderFrontRegardless()
application.run()
PFO_EOF
cat > "$R/windows.swift" <<'PFO_EOF'
import AppKit

let detailed = Set(CommandLine.arguments.dropFirst().compactMap { Int32($0) })
let front = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let list = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
var windows: [[String: Any]] = []
for window in list.prefix(12) {
    let pid = (window[kCGWindowOwnerPID as String] as? Int32) ?? -1
    var entry: [String: Any] = ["pid": pid, "layer": window[kCGWindowLayer as String] ?? -1]
    if detailed.contains(pid) {
        entry["window_id"] = window[kCGWindowNumber as String] ?? -1
        entry["bounds"] = window[kCGWindowBounds as String] ?? [:]
    }
    windows.append(entry)
}
let report: [String: Any] = ["front_pid": front, "z_order_front_to_back": windows]
if let data = try? JSONSerialization.data(withJSONObject: report),
   let text = String(data: data, encoding: .utf8) {
    print(text)
}
PFO_EOF
cat > "$R/ax.swift" <<'PFO_EOF'
import ApplicationServices
import Foundation

let arguments = CommandLine.arguments
guard arguments.count >= 4, let x = Float(arguments[1]), let y = Float(arguments[2]),
      let count = Int(arguments[3]) else { exit(2) }
let applicationPid: pid_t? = arguments.count >= 5 ? pid_t(arguments[4]) : nil
let system = AXUIElementCreateSystemWide()

func role(of element: AXUIElement) -> String {
    var value: CFTypeRef?
    let error = AXUIElementCopyAttributeValue(element, kAXRoleAttribute as CFString, &value)
    guard error == .success, let text = value as? String else { return "error:\(error.rawValue)" }
    return text
}

func hit(_ root: AXUIElement) -> [String: Any] {
    var element: AXUIElement?
    let started = DispatchTime.now().uptimeNanoseconds
    let error = AXUIElementCopyElementAtPosition(root, x, y, &element)
    let elapsed = (DispatchTime.now().uptimeNanoseconds - started) / 1000
    var result: [String: Any] = ["ax_error": Int(error.rawValue), "elapsed_us": Int(elapsed)]
    if error == .success, let found = element {
        var pid: pid_t = -1
        _ = AXUIElementGetPid(found, &pid)
        result["hit_pid"] = Int(pid)
        result["hit_role"] = role(of: found)
    }
    return result
}

func focusedApplication() -> [String: Any] {
    var value: CFTypeRef?
    let started = DispatchTime.now().uptimeNanoseconds
    let error = AXUIElementCopyAttributeValue(system, kAXFocusedApplicationAttribute as CFString, &value)
    let elapsed = (DispatchTime.now().uptimeNanoseconds - started) / 1000
    var result: [String: Any] = ["ax_error": Int(error.rawValue), "elapsed_us": Int(elapsed)]
    if error == .success, let found = value, CFGetTypeID(found) == AXUIElementGetTypeID() {
        var pid: pid_t = -1
        _ = AXUIElementGetPid(found as! AXUIElement, &pid)
        result["pid"] = Int(pid)
    }
    return result
}

var samples: [[String: Any]] = []
for index in 0..<count {
    var sample: [String: Any] = [
        "index": index,
        "system_wide_hit": hit(system),
        "focused_application": focusedApplication(),
    ]
    if let pid = applicationPid {
        sample["application_hit"] = hit(AXUIElementCreateApplication(pid))
    }
    samples.append(sample)
    usleep(100_000)
}
let report: [String: Any] = ["trusted": AXIsProcessTrusted(), "x": Double(x), "y": Double(y), "samples": samples]
if let data = try? JSONSerialization.data(withJSONObject: report),
   let text = String(data: data, encoding: .utf8) {
    print(text)
}
PFO_EOF
cat > "$R/probe_hit.py" <<'PFO_EOF'
import json, os, subprocess, sys, time

ROOT = os.environ.get("PFO_ROOT", "/tmp/pfo")
BIN = ROOT + "/src/target/release/agent-desktop"
APP = "AgentDeskFixture"
OTHER = "Calculator"
LOG = open(ROOT + "/probe.log", "a")
T0 = time.monotonic()
overlay = None


def log(event, **fields):
    line = json.dumps({"src": "probe", "event": event, "t_ms": round((time.monotonic() - T0) * 1000, 1),
                       "wall_us": int(time.time() * 1e6), **fields})
    print(line, flush=True)
    LOG.write(line + "\n")
    LOG.flush()


def pid_of(name):
    out = subprocess.run(["pgrep", "-x", name], capture_output=True, text=True).stdout.split()
    return int(out[0]) if len(out) == 1 else None


def windows():
    pids = [str(pid) for pid in (pid_of(APP), pid_of(OTHER), overlay.pid if overlay else None) if pid]
    out = subprocess.run([ROOT + "/pfo-windows", *pids], capture_output=True, text=True).stdout
    try:
        return json.loads(out)
    except ValueError:
        return {"unparsed": out}


def ad(*args, traced=False, poll_front=False):
    env = dict(os.environ)
    if traced:
        env["RUST_LOG"] = "pfo=debug"
    if poll_front:
        env["PFO_POLL_FRONT"] = "1"
    started = time.monotonic()
    proc = subprocess.run([BIN, *args], capture_output=True, text=True, timeout=90, env=env)
    elapsed = round((time.monotonic() - started) * 1000, 1)
    try:
        body = json.loads(proc.stdout)
    except ValueError:
        body = {"unparsed_stdout": proc.stdout}
    events = []
    for line in proc.stderr.splitlines():
        marker = line.find(" pfo: ")
        if marker < 0:
            continue
        try:
            events.append(json.loads(line[marker + 6:]))
        except ValueError:
            events.append({"unparsed": line})
    return body, elapsed, events


def bring_front(name):
    if name == APP:
        ad("focus-window", "--app", APP)
    else:
        subprocess.run(["open", "-a", name])
    target = None
    for _ in range(50):
        target = pid_of(name)
        state = windows()
        if target and state.get("front_pid") == target:
            time.sleep(0.3)
            log("front_reached", app=name, pid=target, windows=windows())
            return True
        time.sleep(0.1)
    log("front_not_reached", app=name, pid=target, windows=windows())
    return False


def wait_fixture_window():
    for _ in range(60):
        body, _, _ = ad("list-windows", "--app", APP)
        if body.get("ok") is True and body.get("data"):
            log("fixture_window_present", count=len(body["data"]))
            return True
        time.sleep(0.5)
    log("fixture_window_missing", last=body)
    return False


def find_ref(name):
    body = {}
    for _ in range(100):
        body, _, _ = ad("find", "--app", APP, "--role", "button", "--name", name, "--exact", "--first")
        match = (body.get("data") or {}).get("match") or {}
        if body.get("ok") is True and match.get("ref_id"):
            return match["ref_id"]
        time.sleep(0.2)
    log("setup_fail", reason=f"{name} not found", last=body)
    sys.exit(1)


def bounds_of(ref):
    body, _, _ = ad("get", ref, "--property", "bounds")
    return (body.get("data") or {}).get("value"), body


def ax_samples(label, point, phase):
    fixture = pid_of(APP)
    out = subprocess.run([ROOT + "/pfo-ax", str(point[0]), str(point[1]), "5", str(fixture or 0)],
                         capture_output=True, text=True).stdout
    try:
        report = json.loads(out)
    except ValueError:
        report = {"unparsed": out}
    log("ax_samples", label=label, phase=phase, fixture_pid=fixture, overlay_pid=overlay.pid if overlay else None,
        calculator_pid=pid_of(OTHER), report=report, windows=windows())


def traced(label, front, command):
    if not wait_fixture_window():
        return
    reached = bring_front(front)
    log("traced_start", label=label, front=front, front_reached=reached, command=command)
    body, elapsed, events = ad("--headed", *command, traced=True)
    for event in events:
        line = json.dumps({"src": "binary", "label": label, **event})
        print(line, flush=True)
        LOG.write(line + "\n")
    LOG.flush()
    log("traced_end", label=label, elapsed_ms=elapsed, ok=body.get("ok"), error=body.get("error"),
        data=body.get("data"), windows_after=windows())


def raise_cycle(label, point):
    if not wait_fixture_window():
        return
    bring_front(OTHER)
    ax_samples(label, point, "before_raise")
    body, elapsed, _ = ad("focus-window", "--app", APP)
    log("raise", label=label, elapsed_ms=elapsed, ok=body.get("ok"), error=body.get("error"), windows=windows())
    time.sleep(0.3)
    ax_samples(label, point, "after_raise")


def main():
    global overlay
    log("start", bin=BIN, fixture_pid=pid_of(APP), calculator_pid=pid_of(OTHER))
    if not wait_fixture_window():
        sys.exit(1)
    ad("focus-window", "--app", APP)
    ad("move-window", "--app", APP, "--x", "40", "--y", "60")
    ad("resize-window", "--app", APP, "--width", "1000", "--height", "700")
    hover = find_ref("hover-target")
    for _ in range(3):
        body, _, _ = ad("scroll-to", hover)
        if body.get("ok") is True:
            break
        time.sleep(0.5)
    time.sleep(0.5)
    hover = find_ref("hover-target")
    box, raw = bounds_of(hover)
    log("targets", hover=hover, hover_bounds=box)
    if not box:
        log("setup_fail", reason="bounds unavailable", hover_get=raw)
        sys.exit(1)
    point = [box["x"] + box["width"] / 2, box["y"] + box["height"] / 2]
    cover = [box["x"] - 6, box["y"] - 6, box["width"] + 12, box["height"] + 12]

    raise_cycle("U uncovered", point)
    traced("U1 fixture-front uncovered hover", APP, ["hover", hover, "--timeout-ms", "3000"])
    traced("U2 calculator-front uncovered hover", OTHER, ["hover", hover, "--timeout-ms", "3000"])

    overlay = subprocess.Popen([ROOT + "/overlay", *[str(value) for value in cover]])
    open(ROOT + "/overlay.pid", "w").write(str(overlay.pid))
    time.sleep(1.5)
    log("overlay_started", pid=overlay.pid, running=overlay.poll() is None, cover=cover, windows=windows())

    raise_cycle("V covered", point)
    traced("V1 fixture-front covered hover", APP, ["hover", hover, "--timeout-ms", "3000"])
    traced("V2 calculator-front covered hover", OTHER, ["hover", hover, "--timeout-ms", "3000"])

    overlay.kill()
    overlay.wait()
    os.remove(ROOT + "/overlay.pid")
    log("overlay_stopped", pid=overlay.pid)
    log("done")


main()
PFO_EOF
swiftc -O -o "$R/overlay" "$R/overlay.swift" -framework AppKit || exit 1
swiftc -O -o "$R/pfo-windows" "$R/windows.swift" -framework AppKit || exit 1
swiftc -O -o "$R/pfo-ax" "$R/ax.swift" -framework ApplicationServices || exit 1
"$B" launch Calculator --activate >/dev/null 2>&1
sleep 1
[ "$calc_was_running" = 0 ] && "$B" close-app Calculator >/dev/null 2>&1
sleep 1
open "$R/fixture/AgentDeskFixture.app"
python3 "$R/probe_hit.py"
echo "probe exit $? ; send $R/probe.log"
```

## #58 (b) hit-test repro: can the 02:23Z `unknown` come back?

Same build: `probe/hit-test-unknown` at `7de03c996715de413e4dcfba7eade3ffc56f37d6` (CI-compiled green). The window-list
fallback stays unbuilt until this decides.

Why: in the 02:23Z probe `hover-target` sat at x=66 (point 113); at 03:09Z it sat at x=124 (point 171). The fixture
source and the window frame were the same both times. This build logs every `unknown` exit of
`crates/macos/src/tree/hit_test.rs`, so a repro names the exit.

Steps:
- **L1-L5:** five fresh fixture launches with the usual setup (move-window 40,60, resize 1000x700, scroll-to
  hover-target). Each launch logs the target's bounds and the CG z-order, runs one traced uncovered hover with the
  fixture in front, then quits the fixture. This shows how often the x=66 layout appears on its own.
- **X**, a sixth launch:
  - **Control round C** at the natural x (≈124): U1 fixture-front uncovered, U2 Calculator-front uncovered, overlay
    on, V1 fixture-front covered, V2 Calculator-front covered, overlay off.
  - Then the widest `scrollarea` (the root ScrollView) is scrolled `--direction right --amount 1`, up to 12 times,
    until the target's x is within 6 px of 66.
  - If reached: **round S** (U1/U2/V1/V2) at that layout.
  - If not, the log shows `scroll_unreachable` or `scrolled_layout reached=false`. The window then moves left by the
    shortfall, so the target lands at screen x≈66, and **round M** runs. Round M matches run 1's screen point, not its
    position within the window; the log says which path ran.

Rules as in the (b) probe:
- a fresh `mktemp -d /tmp/pfo-repro.XXXXXX` directory, left in place; no `rm` (only its own `overlay.pid` is deleted);
- a detached worktree at the SHA, a cargo wait with a 30 min cap, the desktop-space prelude, and a fixture-window wait
  before each step;
- `trap cleanup EXIT` with confirmation lines; Calculator quit only if the probe launched it;
- no osascript, System Events or lsappinfo, and no window titles of other processes.

How to read it:
- Any `hit_test.unknown_exit` event names the exit and its inputs (bounds, clip result, ax_error).
- If no such event appears in any round, and no L1-L5 layout reaches x≈66 on its own, run 1 is called transient and
  (b) closes with no code.

Laptop checks: `bash -n` and py_compile are clean, and the Python dry-ran against stubs (5 launches, the control round
and the move-window fallback). The lead also confirmed no rm, osascript, System Events or lsappinfo. Shell and Swift
parts are identical to the (b) probe; only the Python driver and the mktemp prefix differ.

```bash
#!/usr/bin/env bash
# #58/(b) hit-test repro probe. Mac only, model-free. Builds probe/hit-test-unknown at 7de03c99
# in a detached worktree under a fresh mktemp directory and writes $R/probe.log.
set -u
SHA=7de03c996715de413e4dcfba7eade3ffc56f37d6
REPO="$HOME/Projects/agent-desktop"
if pgrep -x AgentDeskFixture >/dev/null; then echo "STOP: AgentDeskFixture is already running"; exit 1; fi
calc_was_running=0
pgrep -x Calculator >/dev/null && calc_was_running=1
[ "$calc_was_running" = 1 ] && echo "note: Calculator was already running; the probe uses it and leaves it running"
R=$(mktemp -d /tmp/pfo-repro.XXXXXX) || exit 1
export PFO_ROOT="$R"
B="$R/src/target/release/agent-desktop"
mkdir -p "$R/fixture"
echo "probe directory: $R"
cleanup() {
    if [ -f "$R/overlay.pid" ]; then kill "$(cat "$R/overlay.pid")" 2>/dev/null; fi
    pgrep -f "$R/overlay" >/dev/null && echo "cleanup: WARNING overlay still running" || echo "cleanup: overlay not running"
    [ -x "$B" ] && "$B" close-app AgentDeskFixture --force >/dev/null 2>&1
    pgrep -x AgentDeskFixture >/dev/null && echo "cleanup: WARNING AgentDeskFixture still running" || echo "cleanup: AgentDeskFixture not running"
    if [ "$calc_was_running" = 0 ]; then
        [ -x "$B" ] && "$B" close-app Calculator >/dev/null 2>&1
        pgrep -x Calculator >/dev/null && echo "cleanup: WARNING Calculator still running" || echo "cleanup: Calculator quit (the probe launched it)"
    else
        echo "cleanup: Calculator was running before the probe and stays running"
    fi
    git -C "$REPO" worktree remove --force "$R/src" 2>/dev/null
    [ -e "$R/src" ] && echo "cleanup: WARNING worktree $R/src still present" || echo "cleanup: worktree $R/src removed"
    echo "cleanup: directory $R left in place; log $R/probe.log ($(wc -l < "$R/probe.log" 2>/dev/null || echo 0) lines)"
}
trap cleanup EXIT
git -C "$REPO" fetch origin probe/hit-test-unknown || exit 1
git -C "$REPO" worktree add --detach "$R/src" "$SHA" || exit 1
[ "$(git -C "$R/src" rev-parse HEAD)" = "$SHA" ] || { echo "STOP: worktree is not at $SHA"; exit 1; }
waited=0
while pgrep -x cargo >/dev/null; do
    [ "$waited" -ge 1800 ] && { echo "STOP: another cargo still running after 30 min"; exit 1; }
    echo "waiting for another cargo to finish ($waited s)"; sleep 15; waited=$((waited + 15))
done
(cd "$R/src" && CARGO_BUILD_JOBS=4 cargo build --release -p agent-desktop) || exit 1
bash "$R/src/tests/fixture-app/build.sh" "$R/fixture" >/dev/null || exit 1
cat > "$R/overlay.swift" <<'PFO_EOF'
import AppKit

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let numbers = CommandLine.arguments.dropFirst().compactMap { Double($0) }
guard numbers.count == 4, let primary = NSScreen.screens.first else { exit(2) }
let frame = NSRect(x: numbers[0], y: primary.frame.height - numbers[1] - numbers[3],
                   width: numbers[2], height: numbers[3])
let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel],
                    backing: .buffered, defer: false)
panel.level = .floating
panel.hidesOnDeactivate = false
panel.backgroundColor = .systemRed
let cover = NSButton(title: "pfo-cover", target: nil, action: nil)
cover.frame = NSRect(origin: .zero, size: frame.size)
panel.contentView?.addSubview(cover)
panel.orderFrontRegardless()
application.run()
PFO_EOF
cat > "$R/windows.swift" <<'PFO_EOF'
import AppKit

let detailed = Set(CommandLine.arguments.dropFirst().compactMap { Int32($0) })
let front = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let list = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
var windows: [[String: Any]] = []
for window in list.prefix(12) {
    let pid = (window[kCGWindowOwnerPID as String] as? Int32) ?? -1
    var entry: [String: Any] = ["pid": pid, "layer": window[kCGWindowLayer as String] ?? -1]
    if detailed.contains(pid) {
        entry["window_id"] = window[kCGWindowNumber as String] ?? -1
        entry["bounds"] = window[kCGWindowBounds as String] ?? [:]
    }
    windows.append(entry)
}
let report: [String: Any] = ["front_pid": front, "z_order_front_to_back": windows]
if let data = try? JSONSerialization.data(withJSONObject: report),
   let text = String(data: data, encoding: .utf8) {
    print(text)
}
PFO_EOF
cat > "$R/ax.swift" <<'PFO_EOF'
import ApplicationServices
import Foundation

let arguments = CommandLine.arguments
guard arguments.count >= 4, let x = Float(arguments[1]), let y = Float(arguments[2]),
      let count = Int(arguments[3]) else { exit(2) }
let applicationPid: pid_t? = arguments.count >= 5 ? pid_t(arguments[4]) : nil
let system = AXUIElementCreateSystemWide()

func role(of element: AXUIElement) -> String {
    var value: CFTypeRef?
    let error = AXUIElementCopyAttributeValue(element, kAXRoleAttribute as CFString, &value)
    guard error == .success, let text = value as? String else { return "error:\(error.rawValue)" }
    return text
}

func hit(_ root: AXUIElement) -> [String: Any] {
    var element: AXUIElement?
    let started = DispatchTime.now().uptimeNanoseconds
    let error = AXUIElementCopyElementAtPosition(root, x, y, &element)
    let elapsed = (DispatchTime.now().uptimeNanoseconds - started) / 1000
    var result: [String: Any] = ["ax_error": Int(error.rawValue), "elapsed_us": Int(elapsed)]
    if error == .success, let found = element {
        var pid: pid_t = -1
        _ = AXUIElementGetPid(found, &pid)
        result["hit_pid"] = Int(pid)
        result["hit_role"] = role(of: found)
    }
    return result
}

func focusedApplication() -> [String: Any] {
    var value: CFTypeRef?
    let started = DispatchTime.now().uptimeNanoseconds
    let error = AXUIElementCopyAttributeValue(system, kAXFocusedApplicationAttribute as CFString, &value)
    let elapsed = (DispatchTime.now().uptimeNanoseconds - started) / 1000
    var result: [String: Any] = ["ax_error": Int(error.rawValue), "elapsed_us": Int(elapsed)]
    if error == .success, let found = value, CFGetTypeID(found) == AXUIElementGetTypeID() {
        var pid: pid_t = -1
        _ = AXUIElementGetPid(found as! AXUIElement, &pid)
        result["pid"] = Int(pid)
    }
    return result
}

var samples: [[String: Any]] = []
for index in 0..<count {
    var sample: [String: Any] = [
        "index": index,
        "system_wide_hit": hit(system),
        "focused_application": focusedApplication(),
    ]
    if let pid = applicationPid {
        sample["application_hit"] = hit(AXUIElementCreateApplication(pid))
    }
    samples.append(sample)
    usleep(100_000)
}
let report: [String: Any] = ["trusted": AXIsProcessTrusted(), "x": Double(x), "y": Double(y), "samples": samples]
if let data = try? JSONSerialization.data(withJSONObject: report),
   let text = String(data: data, encoding: .utf8) {
    print(text)
}
PFO_EOF
cat > "$R/probe_repro.py" <<'PFO_EOF'
import json, os, subprocess, sys, time

ROOT = os.environ.get("PFO_ROOT", "/tmp/pfo")
BIN = ROOT + "/src/target/release/agent-desktop"
APP = "AgentDeskFixture"
OTHER = "Calculator"
LOG = open(ROOT + "/probe.log", "a")
T0 = time.monotonic()
overlay = None


def log(event, **fields):
    line = json.dumps({"src": "probe", "event": event, "t_ms": round((time.monotonic() - T0) * 1000, 1),
                       "wall_us": int(time.time() * 1e6), **fields})
    print(line, flush=True)
    LOG.write(line + "\n")
    LOG.flush()


def pid_of(name):
    out = subprocess.run(["pgrep", "-x", name], capture_output=True, text=True).stdout.split()
    return int(out[0]) if len(out) == 1 else None


def windows():
    pids = [str(pid) for pid in (pid_of(APP), pid_of(OTHER), overlay.pid if overlay else None) if pid]
    out = subprocess.run([ROOT + "/pfo-windows", *pids], capture_output=True, text=True).stdout
    try:
        return json.loads(out)
    except ValueError:
        return {"unparsed": out}


def ad(*args, traced=False, poll_front=False):
    env = dict(os.environ)
    if traced:
        env["RUST_LOG"] = "pfo=debug"
    if poll_front:
        env["PFO_POLL_FRONT"] = "1"
    started = time.monotonic()
    proc = subprocess.run([BIN, *args], capture_output=True, text=True, timeout=90, env=env)
    elapsed = round((time.monotonic() - started) * 1000, 1)
    try:
        body = json.loads(proc.stdout)
    except ValueError:
        body = {"unparsed_stdout": proc.stdout}
    events = []
    for line in proc.stderr.splitlines():
        marker = line.find(" pfo: ")
        if marker < 0:
            continue
        try:
            events.append(json.loads(line[marker + 6:]))
        except ValueError:
            events.append({"unparsed": line})
    return body, elapsed, events


def bring_front(name):
    if name == APP:
        ad("focus-window", "--app", APP)
    else:
        subprocess.run(["open", "-a", name])
    target = None
    for _ in range(50):
        target = pid_of(name)
        state = windows()
        if target and state.get("front_pid") == target:
            time.sleep(0.3)
            log("front_reached", app=name, pid=target, windows=windows())
            return True
        time.sleep(0.1)
    log("front_not_reached", app=name, pid=target, windows=windows())
    return False


def wait_fixture_window():
    for _ in range(60):
        body, _, _ = ad("list-windows", "--app", APP)
        if body.get("ok") is True and body.get("data"):
            log("fixture_window_present", count=len(body["data"]))
            return True
        time.sleep(0.5)
    log("fixture_window_missing", last=body)
    return False


def find_ref(name):
    body = {}
    for _ in range(100):
        body, _, _ = ad("find", "--app", APP, "--role", "button", "--name", name, "--exact", "--first")
        match = (body.get("data") or {}).get("match") or {}
        if body.get("ok") is True and match.get("ref_id"):
            return match["ref_id"]
        time.sleep(0.2)
    log("setup_fail", reason=f"{name} not found", last=body)
    sys.exit(1)


def bounds_of(ref):
    body, _, _ = ad("get", ref, "--property", "bounds")
    return (body.get("data") or {}).get("value"), body


def ax_samples(label, point, phase):
    fixture = pid_of(APP)
    out = subprocess.run([ROOT + "/pfo-ax", str(point[0]), str(point[1]), "5", str(fixture or 0)],
                         capture_output=True, text=True).stdout
    try:
        report = json.loads(out)
    except ValueError:
        report = {"unparsed": out}
    log("ax_samples", label=label, phase=phase, fixture_pid=fixture, overlay_pid=overlay.pid if overlay else None,
        calculator_pid=pid_of(OTHER), report=report, windows=windows())


def traced(label, front, command):
    if not wait_fixture_window():
        return
    reached = bring_front(front)
    log("traced_start", label=label, front=front, front_reached=reached, command=command)
    body, elapsed, events = ad("--headed", *command, traced=True)
    for event in events:
        line = json.dumps({"src": "binary", "label": label, **event})
        print(line, flush=True)
        LOG.write(line + "\n")
    LOG.flush()
    log("traced_end", label=label, elapsed_ms=elapsed, ok=body.get("ok"), error=body.get("error"),
        data=body.get("data"), windows_after=windows())


def start_overlay(box):
    global overlay
    cover = [box["x"] - 6, box["y"] - 6, box["width"] + 12, box["height"] + 12]
    overlay = subprocess.Popen([ROOT + "/overlay", *[str(value) for value in cover]])
    open(ROOT + "/overlay.pid", "w").write(str(overlay.pid))
    time.sleep(1.5)
    log("overlay_started", pid=overlay.pid, running=overlay.poll() is None, cover=cover, windows=windows())


def stop_overlay():
    global overlay
    if overlay is None:
        return
    overlay.kill()
    overlay.wait()
    os.remove(ROOT + "/overlay.pid")
    log("overlay_stopped", pid=overlay.pid)
    overlay = None


def launch_fixture(label):
    subprocess.run(["open", ROOT + "/fixture/AgentDeskFixture.app"])
    if not wait_fixture_window():
        log("setup_fail", reason="fixture window missing", label=label)
        sys.exit(1)
    ad("focus-window", "--app", APP)
    ad("move-window", "--app", APP, "--x", "40", "--y", "60")
    ad("resize-window", "--app", APP, "--width", "1000", "--height", "700")
    hover = find_ref("hover-target")
    for _ in range(3):
        body, _, _ = ad("scroll-to", hover)
        if body.get("ok") is True:
            break
        time.sleep(0.5)
    time.sleep(0.5)
    hover = find_ref("hover-target")
    box, raw = bounds_of(hover)
    log("layout", label=label, hover=hover, hover_bounds=box, get=None if box else raw, windows=windows())
    return hover, box


def quit_fixture(label):
    ad("close-app", APP, "--force")
    for _ in range(50):
        if pid_of(APP) is None:
            log("fixture_quit", label=label)
            return
        time.sleep(0.1)
    log("setup_fail", reason="fixture did not quit", label=label)
    sys.exit(1)


def layout_round(label, hover, box):
    traced(label + " U1 fixture-front uncovered hover", APP, ["hover", hover, "--timeout-ms", "3000"])
    traced(label + " U2 calculator-front uncovered hover", OTHER, ["hover", hover, "--timeout-ms", "3000"])
    start_overlay(box)
    traced(label + " V1 fixture-front covered hover", APP, ["hover", hover, "--timeout-ms", "3000"])
    traced(label + " V2 calculator-front covered hover", OTHER, ["hover", hover, "--timeout-ms", "3000"])
    stop_overlay()


def root_scroll_area():
    body, _, _ = ad("find", "--app", APP, "--role", "scrollarea")
    matches = (body.get("data") or {}).get("matches") or []
    best = None
    for match in matches:
        ref = match.get("ref_id")
        if not ref:
            continue
        box, _ = bounds_of(ref)
        log("scroll_area_candidate", ref=ref, bounds=box)
        if box and (best is None or box["width"] > best[1]["width"]):
            best = (ref, box)
    return best


def scroll_target_to(hover, wanted_x):
    area = root_scroll_area()
    if area is None:
        log("scroll_unreachable", reason="no scroll area with a ref")
        return hover, bounds_of(hover)[0], False
    for step in range(12):
        hover = find_ref("hover-target")
        box, _ = bounds_of(hover)
        log("scroll_step", step=step, area=area[0], hover_x=box["x"] if box else None)
        if box and abs(box["x"] - wanted_x) <= 6:
            return hover, box, True
        body, _, _ = ad("scroll", area[0], "--direction", "right", "--amount", "1")
        log("scroll_result", step=step, ok=body.get("ok"), error=body.get("error"))
        time.sleep(0.4)
    hover = find_ref("hover-target")
    return hover, bounds_of(hover)[0], False


def main():
    log("start", bin=BIN, calculator_pid=pid_of(OTHER))
    for index in range(1, 6):
        label = f"L{index} fresh launch"
        hover, box = launch_fixture(label)
        if box:
            traced(label + " U1 fixture-front uncovered hover", APP, ["hover", hover, "--timeout-ms", "3000"])
        quit_fixture(label)

    hover, box = launch_fixture("X final launch")
    if not box:
        sys.exit(1)
    layout_round("C control x=%d" % round(box["x"]), hover, box)

    hover, box, reached = scroll_target_to(hover, 66)
    log("scrolled_layout", reached=reached, hover=hover, hover_bounds=box)
    if reached:
        layout_round("S scrolled x=%d" % round(box["x"]), hover, box)
    else:
        ad("focus-window", "--app", APP)
        shift = round(box["x"] - 66) if box else 58
        ad("move-window", "--app", APP, f"--x={40 - shift}", "--y", "60")
        time.sleep(0.5)
        hover = find_ref("hover-target")
        box, _ = bounds_of(hover)
        log("moved_layout", shift=shift, hover=hover, hover_bounds=box, windows=windows())
        if box:
            layout_round("M moved-window x=%d" % round(box["x"]), hover, box)
    log("done")


main()
PFO_EOF
swiftc -O -o "$R/overlay" "$R/overlay.swift" -framework AppKit || exit 1
swiftc -O -o "$R/pfo-windows" "$R/windows.swift" -framework AppKit || exit 1
swiftc -O -o "$R/pfo-ax" "$R/ax.swift" -framework ApplicationServices || exit 1
"$B" launch Calculator --activate >/dev/null 2>&1
sleep 1
[ "$calc_was_running" = 0 ] && "$B" close-app Calculator >/dev/null 2>&1
sleep 1
python3 "$R/probe_repro.py"
echo "probe exit $? ; send $R/probe.log"
```

## #86 ADAPTERS-4 + ADAPTERS-1: a right-click fallback's selection carries into its delivery; the menu gate refuses unless the menu state is known

Head: `d448d78164eb734cdfbafa58a611bd8ab82b41be`, revision 2 (2026-11-10). CI 13/13 (run 42807833451; all 13
menu_gate + chain_menu_steps tests pass in macOS Test 1082/0), gates GREEN 5136/0, CLEAN, draft. Commits:
- `a834bb8b`: the F1 window-server menu gate, as control approved.
- `06fe3a29`: invert; CI run 42803486018 failed exactly `a_capped_search_without_a_window_server_answer_refuses`.
- `d448d781`: revert.

Main is merged at `4f7d3837`; a main merge-in is needed before landing.

crates/macos changed since `5dbbfcc9`, so the earlier -a..-d pass does not carry over: this is a full run.

RUN parts:
- **Part 1:** fixture -a..-d.
- **1e:** a `--headed` right-click on list-item-3, with the selection read via `find --role statictext --native-id
  list-selection`. It HALTs "1e not exercised" if the before-value is empty, the row is off-window, or the selection is
  unchanged.
- **Part 2:** Script Editor -a..-d; skipped if Script Editor was already running.
- **Part D:** a throwaway DenseMenu AppKit app (3,000 NSButtons) built by BUILD and launched by its executable. The
  precondition is that `snapshot --surface menu` reports `surface_search_limit`, else HALT. Then D-a..D-d: D-a, no
  menu open: the step acts. D-c, menu open: the second right-click is not_delivered.
- **Part T:** headless right-click ms, min/median/max of 7 after a warm-up, merge-base main vs head, on the fixture
  and on DenseMenu. This decides whether the gate's window-list read needs reusing (control, 2026-11-10).

No scroll-to anywhere.

Expected: every -a, -c and 1e line PASS; every -b and -d line True; the D precondition line printed; four `T ...`
lines.

Notes for the worker:
- 1e FAIL with `retry=safe` is the product failure this check exists for.
- D-a FAIL with POLICY_DENIED supported_action means the NSButton exposed no AXShowMenu: a setup limit, so report the
  envelope.
- A Part T "sample failed" on dense head while dense base acts means the gate refuses in dense windows (F1 not fixed).

Laptop checks: `bash -n` is clean, and grep finds no rm, System Events, osascript, lsappinfo, Finder or TextEdit.

BUILD (no lock; prints `BUILD-OK R=<dir>`):

```bash
#!/usr/bin/env bash
# BUILD for #86 at d448d78164eb734cdfbafa58a611bd8ab82b41be: run outside the desktop lock
SHA=d448d78164eb734cdfbafa58a611bd8ab82b41be
TAG=ad-a41
set -u
REPO="$HOME/Projects/agent-desktop"
R=$(mktemp -d "${TMPDIR:-/tmp}/$TAG.XXXXXX") || { echo "FAIL-SETUP mktemp"; exit 1; }
mkdir "$R/home" "$R/space" && chmod 700 "$R/home" "$R/space"
echo "scratch dir: $R (nothing is removed; leave it for George)"
git -C "$REPO" fetch -q origin || { echo "FAIL-SETUP fetch"; exit 1; }
[ "$(git -C "$REPO" rev-parse "$SHA^{commit}" 2>/dev/null)" = "$SHA" ] || { echo "FAIL-SETUP $SHA not in the repo"; exit 1; }
DS="$R/desktop_space.sh"
git -C "$REPO" show origin/chore/speed-1.4-tasks:bench-tools/desktop_space.sh \
  | sed -e 's|^r[m] .*AGENT_DESKTOP_HOME.*$|echo "desktop-space home left at $AGENT_DESKTOP_HOME"|' \
        -e 's|mktemp -d /[^)]*|mktemp -d "$SPACE_DIR/space.XXXX"|' > "$DS" \
  || { echo "FAIL-SETUP desktop_space.sh copy"; exit 1; }
if grep -nE '(^|[;&|[:space:]])r[m][[:space:]]' "$DS"; then echo "FAIL-SETUP desktop_space.sh still runs rm"; exit 1; fi
grep -q 'mktemp -d "$SPACE_DIR/space.XXXX"' "$DS" || { echo "FAIL-SETUP desktop_space.sh mktemp line not redirected"; cat "$DS"; exit 1; }
for p in $(grep -oE '/(Users|private)/[^ "()]+' "$DS"); do
  [ -e "$p" ] || { echo "FAIL-SETUP desktop_space.sh path missing: $p"; exit 1; }
done
echo "desktop_space.sh copied to $DS (homes go under $R/space)"
git -C "$REPO" worktree add -q --detach "$R/src" "$SHA" || { echo "FAIL-SETUP worktree"; exit 1; }
while pgrep -x cargo >/dev/null; do sleep 5; done
( cd "$R/src" && CARGO_TARGET_DIR="$R/src/target" cargo build -q --release -p agent-desktop ) || { echo "FAIL-SETUP build"; exit 1; }
B="$R/src/target/release/agent-desktop"
echo "built $("$B" --version) at $(git -C "$R/src" rev-parse HEAD)"
cat > "$R/cgfront.swift" <<'SWIFT'
import CoreGraphics
import Foundation
guard CommandLine.arguments.count == 2, let pid = Int32(CommandLine.arguments[1]) else { exit(2) }
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let list = (CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]]) ?? []
var own = 0
var front: Int32 = -1
for window in list {
    let layer = window[kCGWindowLayer as String] as? Int ?? -1
    let owner = (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value ?? -1
    let alpha = window[kCGWindowAlpha as String] as? Double ?? 0
    let bounds = window[kCGWindowBounds as String] as? [String: Double] ?? [:]
    guard layer == 0, alpha > 0, (bounds["Width"] ?? 0) > 50, (bounds["Height"] ?? 0) > 50 else { continue }
    if front == -1 { front = owner }
    if owner == pid { own += 1 }
}
print("windows=\(own) front=\(front == pid ? "yes" : "no")")
SWIFT
swiftc -O -o "$R/cgfront" "$R/cgfront.swift" 2>"$R/cgfront.log" || { echo "FAIL-SETUP cgfront build"; cat "$R/cgfront.log"; exit 1; }
bash "$R/src/tests/fixture-app/build.sh" "$R/fixture" >"$R/fixture-build.log" 2>&1 || { echo "FAIL-SETUP fixture build"; cat "$R/fixture-build.log"; exit 1; }
BASE=$(git -C "$REPO" merge-base "$SHA" origin/main) || { echo "FAIL-SETUP merge-base"; exit 1; }
git -C "$REPO" worktree add -q --detach "$R/base" "$BASE" || { echo "FAIL-SETUP base worktree"; exit 1; }
while pgrep -x cargo >/dev/null; do sleep 5; done
( cd "$R/base" && CARGO_TARGET_DIR="$R/base/target" cargo build -q --release -p agent-desktop ) || { echo "FAIL-SETUP base build"; exit 1; }
echo "base (merge-base main) built: $("$R/base/target/release/agent-desktop" --version) at $BASE"
mkdir "$R/dense"
cat > "$R/dense/main.swift" <<'SWIFT'
import AppKit

final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}

final class Delegate: NSObject, NSApplicationDelegate {
    var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let window = NSWindow(
            contentRect: NSRect(x: 160, y: 160, width: 900, height: 640),
            styleMask: [.titled, .closable, .resizable],
            backing: .buffered,
            defer: false)
        window.title = "Dense Menu"
        let document = FlippedView(frame: NSRect(x: 0, y: 0, width: 1960, height: 40 + 150 * 24))
        let target = NSButton(title: "Dense Target", target: nil, action: nil)
        target.frame = NSRect(x: 10, y: 8, width: 160, height: 24)
        target.setAccessibilityIdentifier("dense-target")
        let menu = NSMenu(title: "Dense")
        menu.addItem(withTitle: "Dense Choice", action: nil, keyEquivalent: "")
        target.menu = menu
        document.addSubview(target)
        for row in 0..<150 {
            for column in 0..<20 {
                let button = NSButton(title: "b\(row)-\(column)", target: nil, action: nil)
                button.frame = NSRect(x: 10 + column * 97, y: 40 + row * 24, width: 92, height: 22)
                document.addSubview(button)
            }
        }
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 900, height: 640))
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = true
        scroll.documentView = document
        window.contentView = scroll
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }
}

let application = NSApplication.shared
application.setActivationPolicy(.regular)
let delegate = Delegate()
application.delegate = delegate
application.run()
SWIFT
swiftc -O -o "$R/dense/DenseMenu" "$R/dense/main.swift" 2>"$R/dense/build.log" || { echo "FAIL-SETUP DenseMenu build"; cat "$R/dense/build.log"; exit 1; }
echo "DenseMenu built at $R/dense/DenseMenu"
echo "BUILD-OK R=$R"
echo "next, with the desktop lock held: bash run.sh $R"
```

RUN (desktop lock held; save as `run.sh`, then `bash run.sh <R>`; optional second argument: all | checks | timing):

```bash
#!/usr/bin/env bash
# RUN for #86 at d448d78164eb734cdfbafa58a611bd8ab82b41be: run with the desktop lock held; arguments: the dir BUILD printed, then all (default)
SHA=d448d78164eb734cdfbafa58a611bd8ab82b41be
set -u
R="${1:?usage: run.sh <scratch dir printed by BUILD-OK>}"
B="$R/src/target/release/agent-desktop"
[ -x "$B" ] && [ -x "$R/cgfront" ] && [ -d "$R/fixture/AgentDeskFixture.app" ] || { echo "FAIL-SETUP $R is not a finished BUILD dir"; exit 1; }
[ "$(git -C "$R/src" rev-parse HEAD)" = "$SHA" ] || { echo "FAIL-SETUP $R/src is not at $SHA"; exit 1; }
RUNDIR=$(mktemp -d "$R/run.XXXXXX") || { echo "FAIL-SETUP mktemp"; exit 1; }
mkdir "$RUNDIR/home" && chmod 700 "$RUNDIR/home"
export AGENT_DESKTOP_HOME="$RUNDIR/home"
echo "run dir: $RUNDIR (binary $("$B" --version) at $SHA)"
LAUNCHED_PIDS=""
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
cleanup() {
  echo "== cleanup (trap EXIT)"
  for entry in $LAUNCHED_PIDS; do
    name=${entry%%=*}; name=${name//_/ }; pid=${entry##*=}
    kill -0 "$pid" 2>/dev/null || { echo "cleanup: $name pid $pid already gone"; continue; }
    "$B" close-app "$name" >/dev/null 2>&1
    for _ in $(seq 1 25); do kill -0 "$pid" 2>/dev/null || break; sleep 0.2; done
    kill -0 "$pid" 2>/dev/null && kill "$pid" 2>/dev/null
    for _ in $(seq 1 25); do kill -0 "$pid" 2>/dev/null || break; sleep 0.2; done
    kill -0 "$pid" 2>/dev/null && echo "cleanup: $name pid $pid STILL RUNNING" || echo "cleanup: quit $name pid $pid (launched by this script)"
  done
  "$LSREG" -u "$R/fixture/AgentDeskFixture.app" 2>/dev/null
  echo "cleanup: lsregister -u $R/fixture/AgentDeskFixture.app ran"
  echo "cleanup: done. Left for George: $R (worktree $R/src: git -C ~/Projects/agent-desktop worktree remove --force $R/src)"
}
trap cleanup EXIT
halt() { echo "HALT: $*"; exit 3; }
full() { python3 -c 'import json,sys; d=json.load(sys.stdin); print(json.dumps(d, indent=1)) if d.get("ok") is not True else None'; }
jget() { python3 -c 'import json,sys; d=json.load(sys.stdin); print(eval(sys.argv[1]))' "$1"; }
ref_of() { jget '((d.get("data") or {}).get("match") or {}).get("ref_id","")'; }
count_of() { echo "$1" | wc -w | tr -d ' '; }
status_value() { "$B" find --app "$1" --window-id "$2" --role statictext --native-id "$3" --first | jget '((d.get("data") or {}).get("match") or {}).get("value") or ""'; }

[ -z "$(pgrep -x AgentDeskFixture)" ] || { echo "FAIL-SETUP AgentDeskFixture already running (pid $(pgrep -x AgentDeskFixture)); quit it first"; exit 1; }
if pgrep -x Calculator >/dev/null; then
  echo "desktop-space reset SKIPPED: Calculator was already running (pid $(pgrep -x Calculator)) and stays running; make sure no full-screen app is in front"
else
  SPACE_DIR="$R/space" bash "$R/desktop_space.sh"; echo "desktop-space reset ran (homes under $R/space)"
fi

focused_id() { "$B" list-windows --app "$1" | python3 -c 'import json,sys; print(" ".join(w["id"] for w in json.load(sys.stdin).get("data",[]) if w.get("is_focused")))'; }
front_on() {
  APP="$1"; PID="$2"; WID="$3"
  "$B" focus-window --window-id "$WID" | full
  for _ in $(seq 1 50); do
    CG=$("$R/cgfront" "$PID"); F=$(focused_id "$APP")
    [ "$CG" != "${CG%front=yes}" ] && [ "$F" = "$WID" ] && { echo "front: $APP pid $PID window $WID ($CG)"; return 0; }
    sleep 0.2
  done
  echo "FAIL-SETUP $APP window $WID not front: cg='$CG' focused='$F'"; exit 1
}
win_ids() { "$B" list-windows --app "$1" | python3 -c 'import json,sys; t=sys.argv[1].split("|"); print(" ".join(w["id"] for w in json.load(sys.stdin).get("data",[]) if w.get("title") in t))' "$2"; }
wait_ids() { for _ in $(seq 1 50); do v=$(win_ids "$1" "$2"); [ "$(count_of "$v")" -ge "$3" ] && { echo $v; return; }; sleep 0.2; done; echo ""; }
start_fixture() {
  "$R/fixture/AgentDeskFixture.app/Contents/MacOS/AgentDeskFixture" >"$RUNDIR/fixture.log" 2>&1 &
  FPID=$!; LAUNCHED_PIDS="$LAUNCHED_PIDS AgentDeskFixture=$FPID"
  for _ in $(seq 1 50); do
    M=$("$B" list-windows --app AgentDeskFixture | python3 -c 'import json,sys; print(" ".join(w["id"] for w in json.load(sys.stdin).get("data",[]) if w.get("title")!="Duplicate Window"))')
    [ "$(count_of "$M")" -eq 1 ] && break; sleep 0.2
  done
  echo "fixture pid=$FPID M=$M"; [ "$(count_of "$M")" -eq 1 ] || { echo "FAIL-SETUP fixture window"; exit 1; }
  front_on AgentDeskFixture "$FPID" "$M"
}
PARTS="${2:-all}"
first_ok() { python3 -c '
import json,sys
d=json.load(sys.stdin); e=d.get("error") or {}
u = e.get("code")=="APP_UNRESPONSIVE" and (e.get("disposition") or {}).get("delivery")=="delivery_uncertain"
print("PASS" if d.get("ok") is True or u else "FAIL")
if d.get("ok") is not True: print(json.dumps(d, indent=1))'; }
not_sent() { python3 -c '
import json,sys
d=json.load(sys.stdin); e=d.get("error") or {}; dl=(e.get("disposition") or {}).get("delivery")
print("PASS" if d.get("ok") is False and dl=="not_delivered" else "FAIL ok=%s delivery=%s" % (d.get("ok"), dl))
print(json.dumps(d, indent=1))'; }
ok_of() { OUT=$(cat); echo "$OUT" | jget 'd["ok"]'; echo "$OUT" | full; }
on_screen() { python3 -c '
import json,sys
el=(json.loads(sys.argv[1]).get("data") or {}); w=json.loads(sys.argv[2]).get("data") or []
b=el.get("value") if isinstance(el.get("value"),dict) else el.get("bounds") or el
win=[x.get("bounds") for x in w if x.get("id")==sys.argv[3]]
ok=False
try:
    wb=win[0]; ok = b["x"]>=wb["x"] and b["y"]>=wb["y"] and b["x"]+b["width"]<=wb["x"]+wb["width"] and b["y"]+b["height"]<=wb["y"]+wb["height"]
except Exception: pass
print("yes" if ok else "no element=%s window=%s" % (json.dumps(b), json.dumps(win)))' "$1" "$2" "$3"; }

second_menu_check() {
  APP="$1"; PID="$2"; WID="$3"; REF="$4"; LABEL="$5"
  front_on "$APP" "$PID" "$WID"
  echo "$LABEL-a first right-click opens a menu (expect PASS):"; "$B" right-click "$REF" | first_ok
  echo "$LABEL-b menu is open (expect True):"; "$B" wait --menu --app "$APP" --timeout 2000 | ok_of
  echo "$LABEL-c second right-click while the menu is open sends nothing (expect PASS):"; "$B" right-click "$REF" | not_sent
  "$B" press escape --app "$APP" | full
  echo "$LABEL-d menu closed after escape (expect True):"; "$B" wait --menu-closed --app "$APP" --timeout 2000 | ok_of
}

if [ "$PARTS" = all ] || [ "$PARTS" = checks ]; then
echo "== Part 1: AgentDeskFixture (SwiftUI)"
start_fixture
CT=$("$B" find --app AgentDeskFixture --window-id "$M" --role button --name context-target --exact --first | ref_of)
[ -n "$CT" ] || { echo "FAIL-SETUP no context-target ref"; exit 1; }
second_menu_check AgentDeskFixture "$FPID" "$M" "$CT" 1

echo "== 1e: a headed right-click whose selecting fallback changes the list selection"
BEFORE=$(status_value AgentDeskFixture "$M" list-selection); echo "list-selection before: '$BEFORE'"
[ -n "$BEFORE" ] || halt "1e not exercised: list-selection unreadable before the right-click"
[ "$BEFORE" != 3 ] || halt "1e not exercised: list-item-3 already selected"
ROW=$("$B" find --app AgentDeskFixture --window-id "$M" --name list-item-3 --exact --first | ref_of)
[ -n "$ROW" ] || halt "1e not exercised: list-item-3 has no ref"
VIS=$(on_screen "$("$B" get "$ROW" --property bounds)" "$("$B" list-windows --app AgentDeskFixture)" "$M"); echo "list-item-3 on screen: $VIS"
[ "$VIS" = yes ] || halt "1e not exercised: list-item-3 is not inside the window"
front_on AgentDeskFixture "$FPID" "$M"
OUT=$("$B" --headed right-click "$ROW")
for _ in $(seq 1 10); do AFTER=$(status_value AgentDeskFixture "$M" list-selection); [ "$AFTER" != "$BEFORE" ] && break; sleep 0.2; done
echo "list-selection after: '$AFTER'"; echo "right-click envelope:"; echo "$OUT" | python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin), indent=1))'
[ -n "$AFTER" ] && [ "$AFTER" != "$BEFORE" ] || { "$B" press escape --app AgentDeskFixture | full; halt "1e not exercised: selection unchanged (before='$BEFORE' after='$AFTER')"; }
echo "1e a right-click that changed the selection never says retry safe (expect PASS):"
echo "$OUT" | python3 -c '
import json,sys
d=json.load(sys.stdin); e=d.get("error") or {}; r=(e.get("disposition") or {}).get("retry")
print("PASS (ok true)" if d.get("ok") is True else ("PASS (retry unsafe)" if r=="unsafe" else "FAIL ok=%s retry=%s code=%s" % (d.get("ok"), r, e.get("code"))))'
"$B" press escape --app AgentDeskFixture | full
"$B" wait --menu-closed --app AgentDeskFixture --timeout 2000 | full

echo "== Part 2: Script Editor (AppKit)"
if pgrep -x "Script Editor" >/dev/null; then
  echo "Part 2 SKIP: Script Editor was already running (pid $(pgrep -x "Script Editor")); it stays running and untouched"
else
  printf 'return 1\n' > "$RUNDIR/a.applescript"
  open -a "Script Editor" "$RUNDIR/a.applescript"
  for _ in $(seq 1 50); do SEPID=$(pgrep -x "Script Editor"); [ -n "$SEPID" ] && break; sleep 0.2; done
  [ -n "$SEPID" ] || { echo "FAIL-SETUP Script Editor did not start"; exit 1; }
  LAUNCHED_PIDS="$LAUNCHED_PIDS Script_Editor=$SEPID"
  A=$(wait_ids "Script Editor" "a.applescript|a" 1)
  echo "Script Editor pid=$SEPID A=$A"; [ -n "$A" ] || { echo "FAIL-SETUP Script Editor window"; exit 1; }
  front_on "Script Editor" "$SEPID" "$A"
  TV=$("$B" find --app "Script Editor" --window-id "$A" --role textfield --first | ref_of)
  [ -n "$TV" ] || { echo "FAIL-SETUP no text view ref"; exit 1; }
  second_menu_check "Script Editor" "$SEPID" "$A" "$TV" 2
fi
fi

start_dense() {
  "$R/dense/DenseMenu" >"$RUNDIR/dense.log" 2>&1 &
  DPID=$!; LAUNCHED_PIDS="$LAUNCHED_PIDS DenseMenu=$DPID"
  for _ in $(seq 1 75); do DW=$(win_ids DenseMenu "Dense Menu"); [ "$(count_of "$DW")" -eq 1 ] && break; sleep 0.2; done
  echo "DenseMenu pid=$DPID window=$DW"; [ "$(count_of "$DW")" -eq 1 ] || { echo "FAIL-SETUP DenseMenu window"; exit 1; }
  front_on DenseMenu "$DPID" "$DW"
}

if [ "$PARTS" = all ] || [ "$PARTS" = checks ]; then
echo "== Part D: dense AppKit window (3,000 buttons; the menu search passes its 2,048-node cap)"
start_dense
PRE=$("$B" snapshot --app DenseMenu --surface menu); echo "$PRE" | full
case "$PRE" in *surface_search_limit*) echo "dense precondition: the menu search hits its node cap with no menu open";; *) halt "dense precondition not met: snapshot --surface menu did not report surface_search_limit";; esac
DT=$("$B" find --app DenseMenu --window-id "$DW" --role button --name "Dense Target" --exact --first | ref_of)
[ -n "$DT" ] || { echo "FAIL-SETUP no Dense Target ref"; exit 1; }
second_menu_check DenseMenu "$DPID" "$DW" "$DT" D
fi

if [ "$PARTS" = all ] || [ "$PARTS" = timing ]; then
echo "== Part T: headless right-click latency, merge-base main vs this head (min/median of 7, warm-up discarded)"
BB="$R/base/target/release/agent-desktop"
[ -x "$BB" ] || { echo "FAIL-SETUP no base binary"; exit 1; }
bench() {
  LABEL="$1"; BIN="$2"; APP="$3"; PID="$4"; WID="$5"; NAME="$6"
  REF=$("$BIN" find --app "$APP" --window-id "$WID" --role button --name "$NAME" --exact --first | ref_of)
  [ -n "$REF" ] || { echo "T $LABEL: FAIL-SETUP no ref"; return; }
  : > "$RUNDIR/samples"
  for i in $(seq 0 7); do
    front_on "$APP" "$PID" "$WID" >/dev/null
    python3 -c '
import json,subprocess,sys,time
t=time.perf_counter(); p=subprocess.run(sys.argv[2:],capture_output=True,text=True); ms=(time.perf_counter()-t)*1000
d=json.loads(p.stdout or "{}")
if d.get("ok") is True or (d.get("error") or {}).get("code")=="APP_UNRESPONSIVE":
    open(sys.argv[1],"a").write("%.1f\n" % ms)
else:
    print("sample failed:", json.dumps(d, indent=1))' "$RUNDIR/samples" "$BIN" right-click "$REF"
    "$BIN" press escape --app "$APP" >/dev/null
    "$BIN" wait --menu-closed --app "$APP" --timeout 2000 >/dev/null
  done
  python3 -c '
import statistics,sys
rows=[float(l) for l in open(sys.argv[1]) if l.strip()][1:]
print("T %s: n=%d min=%.1f ms median=%.1f ms max=%.1f ms" % (sys.argv[2], len(rows), min(rows), statistics.median(rows), max(rows)) if rows else "T %s: no valid samples" % sys.argv[2])' "$RUNDIR/samples" "$LABEL"
}
if [ -z "${FPID:-}" ] || ! kill -0 "$FPID" 2>/dev/null; then start_fixture; fi
bench "fixture base" "$BB" AgentDeskFixture "$FPID" "$M" context-target
bench "fixture head" "$B" AgentDeskFixture "$FPID" "$M" context-target
if [ -z "${DPID:-}" ] || ! kill -0 "$DPID" 2>/dev/null; then start_dense; fi
bench "dense base" "$BB" DenseMenu "$DPID" "$DW" "Dense Target"
bench "dense head" "$B" DenseMenu "$DPID" "$DW" "Dense Target"
fi
echo "== checks done"
```

## #58 ACTION-12 (a) cap-1: a headed hover or drag raises its window at most once per command

Head: `063d9229da4a488f6a47d0a2fad0f4734a27425f`. CI 13/13 (run 42807214725, macOS Test included), gates GREEN 5138/0
(post-fix gates.sh). Draft and MERGEABLE. The canonical copy of this section is the PR body's "Mac check" section.

What changed since the probe plan:
- Main is merged up to `9132aa89`, including #99 and #102. Nothing in #58's files reads the front-window
  classification.
- One CommandFocus per command is shared by the pre-lease wait's scroll and the leased attempts, so a raise during the
  wait is the command's one raise.
- A focus call that failed after it may have raised ends the command. Every error before input carries
  preparatory_focus.

The per-app-hit invariant ("never a primary read") can only live in `hit_test.rs`, so it moves to (b)'s plan if (b) is
ever built.

How it runs:
- **BUILD** (outside the lock) builds the head and its merge-base with main, each in its own detached worktree under
  `mktemp -d /tmp/ad-58.XXXXXX` with its own target dir and a cargo wait. It also builds the fixture and two Swift
  helpers (a floating panel, and a CGWindowList reader that prints pids only), copies desktop_space.sh pointed at the
  check dir with its rm line removed, and records the dir in `/tmp/ad-58.last`.
- **RUN** (inside the lock):
  - skips the desktop reset if Calculator is running, and launches the fixture by its executable;
  - gives each binary its own `AGENT_DESKTOP_HOME`;
  - counts raises from the shipped session trace (`input.focus_window`) in a fresh session per covered hover;
  - HALTs unless an uncovered head hover logs exactly 1 raise (the counter is live);
  - its trap undoes everything with confirmation lines and leaves the dir for George.

Expected:
- **Uncovered hover**, fixture in front, warm-up plus 7 runs per binary alternating main and head: every run ok:true;
  head min <= main min + 5 ms and head median <= main median + 5 ms.
- **Covered hover with this head**, 2 runs with Calculator in front and 2 with the fixture in front: exactly 1
  `input.focus_window` event; ok:false with TIMEOUT or ACTION_FAILED and "occluded"; details.preparatory_focus set;
  delivery not_delivered.
- **Covered hover with main**: logged for contrast (expect many raises).
- Last line: `RESULT: ALL PASS (18 checks)`.

Report that line, any FAIL or HALT line, and the `uncovered_stats` line verbatim.

Laptop checks:
- `bash -n` is clean on both blocks, and grep finds no rm, osascript, System Events, lsappinfo, Finder or TextEdit.
- The agent's stub dry-run gave ALL PASS (18), and a stub that raises 18 times failed exactly the 4 raise checks.
- It has not run on a Mac yet.

BUILD (outside the desktop lock):

```bash
#!/usr/bin/env bash
# #58 (a) Mac check, BUILD block: run OUTSIDE the desktop lock. Model-free.
# Builds this head and its merge-base with main in two detached worktrees, the fixture and the helpers.
# Prints "BUILD_OK R=<dir>" and records the dir in /tmp/ad-58.last for the RUN block.
set -u
HEAD_SHA=063d9229da4a488f6a47d0a2fad0f4734a27425f
REPO="$HOME/Projects/agent-desktop"
halt() { echo "HALT: $*"; exit 2; }
R=$(mktemp -d /tmp/ad-58.XXXXXX) || halt "mktemp"
echo "check dir: $R (left for George)"
mkdir -p "$R/fixture" "$R/home-head" "$R/home-main" && chmod 700 "$R/home-head" "$R/home-main" || halt "mkdir"
built=0
cleanup() {
    [ "$built" = 1 ] && { echo "build cleanup: worktrees kept for the RUN block"; return; }
    for tree in head main; do
        [ -e "$R/$tree" ] || continue
        git -C "$REPO" worktree remove --force "$R/$tree" 2>/dev/null
        [ -e "$R/$tree" ] && echo "build cleanup: WARNING worktree $R/$tree still present" || echo "build cleanup: worktree $R/$tree removed"
    done
}
trap cleanup EXIT
git -C "$REPO" fetch -q origin fix/pointer-focus-once main chore/speed-1.4-tasks || halt "fetch"
[ "$(git -C "$REPO" rev-parse "$HEAD_SHA^{commit}" 2>/dev/null)" = "$HEAD_SHA" ] || halt "$HEAD_SHA not in the repo"
MAIN_SHA=$(git -C "$REPO" merge-base origin/main "$HEAD_SHA") || halt "merge-base"
echo "head $HEAD_SHA; main (merge-base) $MAIN_SHA"
git -C "$REPO" show origin/chore/speed-1.4-tasks:bench-tools/desktop_space.sh \
  | sed -e "s#mktemp -d [^)]*#mktemp -d $R/space.XXXX#" -e '/^rm -rf /d' > "$R/desktop_space.sh" || halt "desktop_space.sh copy"
if grep -nE '(^|[;&|[:space:]])r[m][[:space:]]' "$R/desktop_space.sh"; then halt "desktop_space.sh still runs rm"; fi
grep -q "mktemp -d $R/space.XXXX" "$R/desktop_space.sh" || halt "desktop_space.sh mktemp not pointed at $R"
for p in $(grep -oE '/(Users|private)/[^ "()]+' "$R/desktop_space.sh"); do
    [ -e "$p" ] || halt "desktop_space.sh path missing: $p"
done
git -C "$REPO" worktree add -q --detach "$R/head" "$HEAD_SHA" || halt "head worktree"
git -C "$REPO" worktree add -q --detach "$R/main" "$MAIN_SHA" || halt "main worktree"
[ "$(git -C "$R/head" rev-parse HEAD)" = "$HEAD_SHA" ] || halt "head worktree is not at $HEAD_SHA"
for tree in head main; do
    while pgrep -x cargo >/dev/null; do sleep 5; done
    (cd "$R/$tree" && CARGO_TARGET_DIR="$R/$tree/target" CARGO_BUILD_JOBS=4 cargo build --release -p agent-desktop) || halt "cargo build $tree"
    [ -x "$R/$tree/target/release/agent-desktop" ] || halt "no binary for $tree"
done
bash "$R/head/tests/fixture-app/build.sh" "$R/fixture" >/dev/null || halt "fixture build"
cat > "$R/overlay.swift" <<'PFO_EOF'
import AppKit

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let numbers = CommandLine.arguments.dropFirst().compactMap { Double($0) }
guard numbers.count == 4, let primary = NSScreen.screens.first else { exit(2) }
let frame = NSRect(x: numbers[0], y: primary.frame.height - numbers[1] - numbers[3],
                   width: numbers[2], height: numbers[3])
let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel],
                    backing: .buffered, defer: false)
panel.level = .floating
panel.hidesOnDeactivate = false
panel.backgroundColor = .systemRed
let cover = NSButton(title: "pfo-cover", target: nil, action: nil)
cover.frame = NSRect(origin: .zero, size: frame.size)
panel.contentView?.addSubview(cover)
panel.orderFrontRegardless()
application.run()
PFO_EOF
cat > "$R/windows.swift" <<'PFO_EOF'
import AppKit

let detailed = Set(CommandLine.arguments.dropFirst().compactMap { Int32($0) })
let front = NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
let list = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
var windows: [[String: Any]] = []
for window in list.prefix(12) {
    let pid = (window[kCGWindowOwnerPID as String] as? Int32) ?? -1
    var entry: [String: Any] = ["pid": pid, "layer": window[kCGWindowLayer as String] ?? -1]
    if detailed.contains(pid) {
        entry["window_id"] = window[kCGWindowNumber as String] ?? -1
        entry["bounds"] = window[kCGWindowBounds as String] ?? [:]
    }
    windows.append(entry)
}
let report: [String: Any] = ["front_pid": front, "z_order_front_to_back": windows]
if let data = try? JSONSerialization.data(withJSONObject: report),
   let text = String(data: data, encoding: .utf8) {
    print(text)
}
PFO_EOF
cat > "$R/proof.py" <<'PFO_EOF'
import json, os, statistics, subprocess, sys, time

ROOT = os.environ["PFO_ROOT"]
BINS = {"main": ROOT + "/main/target/release/agent-desktop", "head": ROOT + "/head/target/release/agent-desktop"}
APP = "AgentDeskFixture"
OTHER = "Calculator"
LOG = open(ROOT + "/proof.log", "a")
T0 = time.monotonic()
overlay = None
results = []


def log(event, **fields):
    line = json.dumps({"event": event, "t_ms": round((time.monotonic() - T0) * 1000, 1), **fields})
    print(line, flush=True)
    LOG.write(line + "\n")
    LOG.flush()


def record(label, passed, **fields):
    results.append((label, passed))
    log("check", label=label, result="PASS" if passed else "FAIL", **fields)


def pid_of(name):
    out = subprocess.run(["pgrep", "-x", name], capture_output=True, text=True).stdout.split()
    return int(out[0]) if len(out) == 1 else None


def front_pid():
    out = subprocess.run([ROOT + "/pfo-windows"], capture_output=True, text=True).stdout
    try:
        return json.loads(out).get("front_pid")
    except ValueError:
        return None


def ad(which, *args):
    env = dict(os.environ)
    env["AGENT_DESKTOP_HOME"] = ROOT + "/home-" + which
    started = time.monotonic()
    proc = subprocess.run([BINS[which], *args], capture_output=True, text=True, timeout=90, env=env)
    elapsed = round((time.monotonic() - started) * 1000, 1)
    try:
        body = json.loads(proc.stdout)
    except ValueError:
        body = {"unparsed_stdout": proc.stdout, "stderr": proc.stderr}
    return body, elapsed


def bring_front(name):
    if name == APP:
        ad("head", "focus-window", "--app", APP)
    else:
        subprocess.run(["open", "-a", name])
    for _ in range(50):
        target = pid_of(name)
        if target and front_pid() == target:
            time.sleep(0.3)
            return True
        time.sleep(0.1)
    log("setup_fail", reason=f"{name} did not come to the front")
    sys.exit(1)


def wait_fixture_window():
    body = {}
    for _ in range(60):
        body, _ = ad("head", "list-windows", "--app", APP)
        if body.get("ok") is True and body.get("data"):
            return
        time.sleep(0.5)
    log("setup_fail", reason="fixture window missing", last=body)
    sys.exit(1)


def find_ref(which, name):
    body = {}
    for _ in range(100):
        body, _ = ad(which, "find", "--app", APP, "--role", "button", "--name", name, "--exact", "--first")
        match = (body.get("data") or {}).get("match") or {}
        if body.get("ok") is True and match.get("ref_id"):
            return match["ref_id"]
        time.sleep(0.2)
    log("setup_fail", reason=f"{which}: {name} not found", last=body)
    sys.exit(1)


def covered_run(which, ref, front, index):
    wait_fixture_window()
    bring_front(front)
    session, _ = ad(which, "session", "start", "--name", f"proof-{which}-{index}")
    session_id = (session.get("data") or {}).get("session_id")
    if not session_id:
        log("setup_fail", reason="session start failed", last=session)
        sys.exit(1)
    body, elapsed = ad(which, "--session", session_id, "--headed", "hover", ref, "--timeout-ms", "3000")
    trace, _ = ad(which, "--session", session_id, "trace", "show", "--event", "input.focus_window", "--limit", "0")
    raises = len((trace.get("data") or {}).get("events") or [])
    ad(which, "session", "end", session_id)
    error = body.get("error") or {}
    if trace.get("ok") is not True:
        log("halt", reason="trace show failed", envelope=trace)
        sys.exit(2)
    log("covered_run", binary=which, front=front, index=index, elapsed_ms=elapsed, raises=raises,
        envelope=body)
    return body, error, raises, elapsed


def uncovered_time(which, ref):
    ad(which, "--headed", "mouse-move", "--xy", "20,20")
    bring_front(APP)
    body, elapsed = ad(which, "--headed", "hover", ref, "--timeout-ms", "3000")
    return body.get("ok") is True, elapsed, body.get("error")


def main():
    global overlay
    log("start", bins=BINS)
    wait_fixture_window()
    ad("head", "focus-window", "--app", APP)
    ad("head", "move-window", "--app", APP, "--x", "40", "--y", "60")
    ad("head", "resize-window", "--app", APP, "--width", "1000", "--height", "700")
    head_ref = find_ref("head", "hover-target")
    for _ in range(3):
        body, _ = ad("head", "scroll-to", head_ref)
        if body.get("ok") is True:
            break
        time.sleep(0.5)
    time.sleep(0.5)
    refs = {"head": find_ref("head", "hover-target"), "main": find_ref("main", "hover-target")}
    bounds, _ = ad("head", "get", refs["head"], "--property", "bounds")
    box = (bounds.get("data") or {}).get("value")
    if not box:
        log("setup_fail", reason="bounds unavailable", last=bounds)
        sys.exit(1)
    log("target", refs=refs, bounds=box)

    refs["head"] = find_ref("head", "hover-target")
    body, error, raises, elapsed = covered_run("head", refs["head"], OTHER, 0)
    if body.get("ok") is not True or raises != 1:
        log("halt", reason="raise counter is not live: an uncovered head hover with Calculator in front must log exactly one input.focus_window event",
            raises=raises, envelope=body)
        sys.exit(2)
    log("counter_live", raises=raises)

    samples = {"main": [], "head": []}
    for which in ("main", "head"):
        uncovered_time(which, refs[which])
    for round_index in range(7):
        for which in ("main", "head"):
            refs[which] = find_ref(which, "hover-target")
            ok, elapsed, error = uncovered_time(which, refs[which])
            log("uncovered_run", binary=which, round=round_index, ok=ok, elapsed_ms=elapsed, error=error)
            if not ok:
                record(f"uncovered hover ok ({which}, round {round_index})", False, error=error)
            samples[which].append(elapsed)
    stats = {which: {"min": min(values), "median": statistics.median(values), "max": max(values)}
             for which, values in samples.items()}
    log("uncovered_stats", stats=stats, samples=samples)
    record("uncovered hover min: head <= main + 5 ms", stats["head"]["min"] <= stats["main"]["min"] + 5,
           head=stats["head"]["min"], main=stats["main"]["min"])
    record("uncovered hover median: head <= main + 5 ms", stats["head"]["median"] <= stats["main"]["median"] + 5,
           head=stats["head"]["median"], main=stats["main"]["median"])

    cover = [box["x"] - 6, box["y"] - 6, box["width"] + 12, box["height"] + 12]
    overlay = subprocess.Popen([ROOT + "/overlay", *[str(value) for value in cover]])
    open(ROOT + "/overlay.pid", "w").write(str(overlay.pid))
    time.sleep(1.5)
    log("overlay_started", pid=overlay.pid, running=overlay.poll() is None, cover=cover)

    for front in (OTHER, APP):
        for index in range(1, 3):
            refs["head"] = find_ref("head", "hover-target")
            body, error, raises, elapsed = covered_run("head", refs["head"], front, index)
            text = json.dumps(error)
            record(f"head covered hover, {front} in front, run {index}: one raise", raises == 1, raises=raises)
            record(f"head covered hover, {front} in front, run {index}: refused as occluded",
                   body.get("ok") is False and error.get("code") in ("TIMEOUT", "ACTION_FAILED")
                   and "occluded" in text, code=error.get("code"))
            record(f"head covered hover, {front} in front, run {index}: preparatory_focus",
                   "preparatory_focus" in (error.get("details") or {}))
            record(f"head covered hover, {front} in front, run {index}: not delivered",
                   (error.get("disposition") or {}).get("delivery") == "not_delivered",
                   disposition=error.get("disposition"))
        refs["main"] = find_ref("main", "hover-target")
        body, error, raises, elapsed = covered_run("main", refs["main"], front, 1)
        log("main_covered_reference", front=front, raises=raises, code=error.get("code"))

    overlay.kill()
    overlay.wait()
    os.remove(ROOT + "/overlay.pid")
    overlay = None
    failed = [label for label, passed in results if not passed]
    print(f"RESULT: {'ALL PASS' if not failed else str(len(failed)) + ' FAIL'} ({len(results)} checks)", flush=True)
    LOG.write(json.dumps({"event": "result", "failed": failed, "checks": len(results)}) + "\n")
    sys.exit(0 if not failed else 1)


main()
PFO_EOF
swiftc -O -o "$R/overlay" "$R/overlay.swift" -framework AppKit || halt "swiftc overlay"
swiftc -O -o "$R/pfo-windows" "$R/windows.swift" -framework AppKit || halt "swiftc windows helper"
built=1
printf '%s\n' "$R" > /tmp/ad-58.last || halt "cannot record the check dir"
echo "BUILD_OK R=$R (recorded in /tmp/ad-58.last for the RUN block)"
```

RUN (inside the desktop lock, after BUILD printed BUILD_OK):

```bash
#!/usr/bin/env bash
# #58 (a) Mac check, RUN block: run INSIDE the desktop lock, after BUILD printed BUILD_OK.
set -u
R=$(cat /tmp/ad-58.last 2>/dev/null)
[ -n "$R" ] && [ -d "$R" ] || { echo "HALT: no check dir in /tmp/ad-58.last; run the BUILD block first"; exit 2; }
echo "check dir: $R"
REPO="$HOME/Projects/agent-desktop"
B="$R/head/target/release/agent-desktop"
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
halt() { echo "HALT: $*"; exit 2; }
[ -x "$B" ] && [ -x "$R/main/target/release/agent-desktop" ] && [ -x "$R/pfo-windows" ] || halt "BUILD did not finish in $R"
[ "$(pgrep -x AgentDeskFixture)" = "" ] || halt "AgentDeskFixture already running (pid $(pgrep -x AgentDeskFixture)); quit it first"
calc_was_running=0
pgrep -x Calculator >/dev/null && calc_was_running=1
[ "$calc_was_running" = 1 ] && echo "NOTE Calculator was already running; the check uses it and leaves it running"
export PFO_ROOT="$R"
fixture_pid=""
cleanup() {
    echo "== cleanup (trap EXIT)"
    if [ -f "$R/overlay.pid" ]; then kill "$(cat "$R/overlay.pid")" 2>/dev/null; fi
    pgrep -f "$R/overlay" >/dev/null && echo "cleanup: WARNING overlay still running" || echo "cleanup: overlay not running"
    if [ -n "$fixture_pid" ]; then
        AGENT_DESKTOP_HOME="$R/home-head" "$B" close-app AgentDeskFixture --force >/dev/null 2>&1
        for _ in $(seq 1 25); do kill -0 "$fixture_pid" 2>/dev/null || break; sleep 0.2; done
        kill -0 "$fixture_pid" 2>/dev/null && kill "$fixture_pid" 2>/dev/null
        kill -0 "$fixture_pid" 2>/dev/null && echo "cleanup: WARNING AgentDeskFixture pid $fixture_pid still running" || echo "cleanup: quit AgentDeskFixture pid $fixture_pid (launched by this check)"
    fi
    "$LSREG" -u "$R/fixture/AgentDeskFixture.app" 2>/dev/null; echo "cleanup: lsregister -u $R/fixture/AgentDeskFixture.app ran"
    if [ "$calc_was_running" = 0 ]; then
        AGENT_DESKTOP_HOME="$R/home-head" "$B" close-app Calculator >/dev/null 2>&1
        pgrep -x Calculator >/dev/null && echo "cleanup: WARNING Calculator still running" || echo "cleanup: Calculator quit (this check launched it)"
    else
        echo "cleanup: Calculator was running before the check and stays running"
    fi
    for tree in head main; do
        git -C "$REPO" worktree remove --force "$R/$tree" 2>/dev/null
        [ -e "$R/$tree" ] && echo "cleanup: WARNING worktree $R/$tree still present" || echo "cleanup: worktree $R/$tree removed"
    done
    echo "cleanup: done; $R left for George (proof.log inside)"
}
trap cleanup EXIT
if [ "$calc_was_running" = 1 ]; then echo "NOTE desktop-space reset skipped (it would quit Calculator)"
else bash "$R/desktop_space.sh"; echo "desktop-space reset ran (exit $?)"; fi
"$R/fixture/AgentDeskFixture.app/Contents/MacOS/AgentDeskFixture" >/dev/null 2>&1 &
fixture_pid=$!
echo "fixture launched by its executable, pid $fixture_pid"
python3 "$R/proof.py"
echo "check exit $?; send $R/proof.log"
```
