# Mac checks (batch for the Mac worker)

Owner: laptop lead. Each section names the PR, the exact head SHA the steps apply to, the steps and the expected
observations. Run a section only if the PR's current head equals the SHA written here; if it differs, this file is stale
for that PR: skip it and say so. Report per PR: head SHA, each check's PASS/FAIL line, and anything unexpected.

Rules learned from the #58 run (2026-11-10), binding for every section:
- Build from a detached worktree under `/tmp` at the SHA (`git -C ~/Projects/agent-desktop worktree add --detach /tmp/<name> <SHA>`); never `git switch` in `~/Projects/agent-desktop`. Wait `while pgrep -x cargo >/dev/null; do sleep 5; done` before building.
- Start with the desktop-space reset (`bench-tools/desktop_space.sh` from `origin/chore/speed-1.4-tasks`; its paths are Mac-local) and wait for the app or fixture window to exist and be front before the first check. A full-screen T3 Code Space made the fixture vanish after ~12 s.
- `find` on a shipped binary omits `bounds` when the match has a ref (`crates/core/src/commands/find_response.rs:61-70`); read bounds with `get <ref> --property bounds`.
- Print full error envelopes, never truncated.
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
| #80 | SURFACE-10 | STALE (`2982cc00`; PR head moved to `db60af6e`, DIRTY) | do not run: being re-merged with main; the SHA below will be updated when the new head is green |
| #58 | ACTION-12 | `d57dadfa` | **FAIL 10/17** (two runs, 2026-11-10): early stop never fires on a real Mac, no `occluded` refusal, lock-timeout label on a deadline expiry. Old steps below are superseded; a `#58 probe` section is coming |
| #86 | ADAPTERS-4 + ADAPTERS-1 | pending | DIRTY; being re-merged; steps pending |
| #88 | LIFECYCLE-2 + LIFECYCLE-12 | `46f397a7` | steps ready, revised per control (no System Events, EXIT trap, Calculator rule); CI 13/13 green |
| #34 | SURFACE-3 | `e3818161` | CI 27/27 green, CLEAN, ready; rev 2 accepted (1 s Escape rule); HELD for George's go-ahead + Script Editor notification settings |
| #18 | SURFACE-1 | pending | starts after #34 lands |

## #80 SURFACE-10: focused-surface ref resolves only in its capture window

Head: `2982cc00c6f35b9227100a266be341dd9eafad3d`. Uses Script Editor (AppKit, two plain-text scripts under `/tmp`) and the
repo's AgentDeskFixture (SwiftUI). Every line must read `PASS`; 1c and 2c must print `True`. On `main` 1a prints `FAIL`
and `ran-b` appears (the click ran document b's script); 2a prints `FAIL` with `saved_ref_absent`.

```bash
set -u
SHA=2982cc00c6f35b9227100a266be341dd9eafad3d
R=/tmp/ad-s10; rm -rf "$R"; mkdir -p "$R/home"
git -C ~/Projects/agent-desktop fetch -q origin
git -C ~/Projects/agent-desktop worktree add -q --detach "$R/src" "$SHA" || { echo "FAIL-SETUP worktree"; exit 1; }
( cd "$R/src" && cargo build -q --release -p agent-desktop ) || { echo "FAIL-SETUP build"; exit 1; }
B="$R/src/target/release/agent-desktop"; export AGENT_DESKTOP_HOME="$R/home"
jget() { python3 -c 'import json,sys; d=json.load(sys.stdin); print(eval(sys.argv[1]))' "$1"; }
refused() { python3 -c 'import json,sys; d=json.load(sys.stdin); e=d.get("error") or {}; print("PASS" if d.get("ok") is False and e.get("code")=="STALE_REF" and "focused_window_changed" in json.dumps(e) else "FAIL")'; }
strip_tokens() { python3 - "$R/home/snapshots/$1/refmap.json" <<'PY'
import json, sys
p = sys.argv[1]; d = json.load(open(p)); n = 0
def walk(x):
    global n
    if isinstance(x, dict):
        if x.pop("retained_object", None) is not None: n += 1
        for v in x.values(): walk(v)
    elif isinstance(x, list):
        for v in x: walk(v)
walk(d); json.dump(d, open(p, "w")); print("tokens stripped:", n)
PY
}
win_ids() { "$B" list-windows --app "$1" | python3 -c 'import json,sys; t=sys.argv[1].split("|"); print(" ".join(w["id"] for w in json.load(sys.stdin).get("data",[]) if w.get("title") in t))' "$2"; }
wait_ids() { for _ in $(seq 1 50); do v=$(win_ids "$1" "$2"); [ "$(echo $v | wc -w)" -ge "$3" ] && { echo $v; return; }; sleep 0.2; done; echo ""; }

echo "== Part 1: Script Editor, two documents"
printf 'do shell script "touch %s/ran-a"\n' "$R" > "$R/a.applescript"
printf 'do shell script "touch %s/ran-b"\n' "$R" > "$R/b.applescript"
open -a "Script Editor" "$R/a.applescript"; A=$(wait_ids "Script Editor" "a.applescript|a" 1)
open -a "Script Editor" "$R/b.applescript"; W2=$(wait_ids "Script Editor" "b.applescript|b" 1)
echo "A=$A B=$W2"; [ -n "$A" ] && [ -n "$W2" ] || { echo "FAIL-SETUP windows"; exit 1; }
"$B" focus-window --window-id "$A" >/dev/null
F=$("$B" find --app "Script Editor" --window-id "$A" --surface focused --role button --name Run --exact --first)
REF=$(echo "$F" | jget 'd["data"]["match"]["ref_id"]'); SNAP=$(echo "$F" | jget 'd["data"]["snapshot_id"]')
echo "ref=$REF snapshot=$SNAP"; strip_tokens "$SNAP"
"$B" focus-window --window-id "$W2" >/dev/null
echo "1a click after focus moved to b (expect PASS):"; "$B" click "$REF" | refused
echo "1b focused find in a while b is focused (expect PASS):"; "$B" find --app "Script Editor" --window-id "$A" --surface focused --role button --name Run --exact --first | refused
"$B" focus-window --window-id "$A" >/dev/null
echo "1c click after a is focused again (expect ok true):"; "$B" click "$REF" | jget 'd["ok"]'
for _ in $(seq 1 50); do [ -e "$R/ran-a" ] && break; sleep 0.2; done
echo "1d ran-a exists (expect PASS): $([ -e "$R/ran-a" ] && echo PASS || echo FAIL)"
echo "1e ran-b absent (expect PASS): $([ -e "$R/ran-b" ] && echo FAIL || echo PASS)"

echo "== Part 2: AgentDeskFixture (SwiftUI)"
bash "$R/src/tests/fixture-app/build.sh" "$R/fixture" >/dev/null || { echo "FAIL-SETUP fixture build"; exit 1; }
open "$R/fixture/AgentDeskFixture.app"
for _ in $(seq 1 50); do M=$("$B" list-windows --app AgentDeskFixture | jget '" ".join(w["id"] for w in d.get("data",[]))'); [ "$(echo $M | wc -w)" = 1 ] && break; sleep 0.2; done
echo "M=$M"; "$B" focus-window --window-id "$M" >/dev/null
status() { "$B" find --app AgentDeskFixture --window-id "$M" --name click-status --exact --first | jget 'd["data"]["match"].get("value")'; }
BEFORE=$(status); echo "click-status before: $BEFORE"
F=$("$B" find --app AgentDeskFixture --window-id "$M" --surface focused --role button --name primary-button --exact --first)
REF2=$(echo "$F" | jget 'd["data"]["match"]["ref_id"]'); strip_tokens "$(echo "$F" | jget 'd["data"]["snapshot_id"]')"
O=$("$B" find --app AgentDeskFixture --window-id "$M" --role button --name open-duplicate-windows --exact --first | jget 'd["data"]["match"]["ref_id"]')
"$B" click "$O" >/dev/null; D=$(wait_ids AgentDeskFixture "Duplicate Window" 2); D1=${D%% *}; echo "D1=$D1"
"$B" focus-window --window-id "$D1" >/dev/null
echo "2a click after focus moved to a duplicate window (expect PASS):"; "$B" click "$REF2" | refused
echo "2b click-status unchanged (expect PASS): $([ "$(status)" = "$BEFORE" ] && echo PASS || echo FAIL)"
"$B" focus-window --window-id "$M" >/dev/null
echo "2c click after main window is focused again (expect ok true):"; "$B" click "$REF2" | jget 'd["ok"]'
for _ in $(seq 1 25); do [ "$(status)" != "$BEFORE" ] && break; sleep 0.2; done
echo "2d click-status changed (expect PASS): $([ "$(status)" != "$BEFORE" ] && echo PASS || echo FAIL)"
C=$("$B" find --app AgentDeskFixture --window-id "$M" --role button --name close-duplicate-windows --exact --first | jget 'd["data"]["match"]["ref_id"]'); "$B" click "$C" >/dev/null
"$B" close-app AgentDeskFixture >/dev/null; "$B" close-app "Script Editor" >/dev/null
echo "== done; scratch worktree left at $R/src"
```

Cleanup: `git -C ~/Projects/agent-desktop worktree remove --force /tmp/ad-s10/src && rm -rf /tmp/ad-s10`.

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

Head: `46f397a712f72d8f01b580c5b4054d1961c0e043`. Save the script as `/tmp/adl-check.sh` and run `bash /tmp/adl-check.sh` from a
terminal with Accessibility permission. Needs Xcode command line tools (swiftc, osacompile) and rustup. It builds in a
detached worktree under `/tmp/adl-check`, waits for running cargo, runs the desktop-space reset, and uses scratch apps in
`/tmp/adl-check` (an AppKit Swift app and an AppleScript applet that quit after 0.5 s for LIFECYCLE-2; an AppKit Swift app
and a JXA stay-open applet with two windows for LIFECYCLE-12) plus Calculator as a control. No Finder, no TextEdit.

Expected after the fix: all 6 cases print PASS (each prints its full envelope first). Before the fix: the LIFECYCLE-2
cases answer `ok: true` with a dead pid (the `--activate` ones after ~20 s), the LIFECYCLE-12 cases `AMBIGUOUS_TARGET`.
INVALID means a fixture precondition was not met (2 windows, none focused); whether macOS makes a restored window key
without activation was not verifiable off-Mac.

Revised 2026-11-10 after control's review: the window count uses a CGWindowList helper (`/tmp/adl-check/wincount`, owner pid + layer 0 only, prints a number) instead of System Events; the four `lsregister -f` registrations are undone in an EXIT trap that prints `UNREGISTER ran for: ...`; if Calculator was already running at the start, the desktop-space reset (which quits Calculator) is skipped and Calculator is left running, with a NOTE line. Delete `/tmp/adl-check` after the run.

```bash
#!/bin/bash
set -u
SHA=46f397a712f72d8f01b580c5b4054d1961c0e043
W=/tmp/adl-check; R=~/Projects/agent-desktop
git -C "$R" worktree remove --force "$W/src" 2>/dev/null; rm -rf "$W"; mkdir -p "$W"; cd "$W" || exit 1
git -C "$R" fetch -q origin && git -C "$R" worktree add -q --detach "$W/src" "$SHA" || exit 1
while pgrep -x cargo >/dev/null; do sleep 5; done
CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR="$W/target" cargo build -q --release -p agent-desktop --manifest-path "$W/src/Cargo.toml" || exit 1
git -C "$R" fetch -q origin chore/speed-1.4-tasks && git -C "$R" show origin/chore/speed-1.4-tasks:bench-tools/desktop_space.sh > "$W/desktop_space.sh"
CALC_WAS_RUNNING=no; pgrep -x Calculator >/dev/null && CALC_WAS_RUNNING=yes
if [ "$CALC_WAS_RUNNING" = yes ]; then echo "NOTE Calculator was already running: skipping the desktop-space reset (it quits Calculator) and leaving Calculator running"
else bash "$W/desktop_space.sh"; fi
AD="$W/target/release/agent-desktop"; export AGENT_DESKTOP_HOME="$W/home"
LSREG=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
REGISTERED=""
unregister_all() {
  for a in $REGISTERED; do "$LSREG" -u "$W/$a.app"; done
  echo "UNREGISTER ran for: ${REGISTERED:-none}"
  REGISTERED=""
}
trap 'pkill -f "$W/Adl" 2>/dev/null; unregister_all' EXIT
cat > "$W/wincount.swift" <<'SWIFT'
import CoreGraphics
import Foundation
guard CommandLine.arguments.count == 2, let pid = Int32(CommandLine.arguments[1]) else { exit(2) }
let info = (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]]) ?? []
let count = info.filter {
    ($0[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == pid && ($0[kCGWindowLayer as String] as? NSNumber)?.intValue == 0
}.count
print(count)
SWIFT
swiftc -O -o "$W/wincount" "$W/wincount.swift" || exit 1
bundle() { # name bundle-id swift-source
  mkdir -p "$W/$1.app/Contents/MacOS"; printf '%s\n' "$3" > "$W/$1.swift"
  swiftc -O -o "$W/$1.app/Contents/MacOS/$1" "$W/$1.swift" -framework AppKit || exit 1
  printf '<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>%s</string><key>CFBundleExecutable</key><string>%s</string><key>CFBundleName</key><string>%s</string><key>CFBundlePackageType</key><string>APPL</string><key>NSPrincipalClass</key><string>NSApplication</string></dict></plist>\n' "$2" "$1" "$1" > "$W/$1.app/Contents/Info.plist"
  "$LSREG" -f "$W/$1.app" && REGISTERED="$REGISTERED $1"
}
bundle AdlQuit dev.adl.check.quit 'import AppKit
final class D: NSObject, NSApplicationDelegate { func applicationDidFinishLaunching(_ n: Notification) { DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { exit(0) } } }
let a = NSApplication.shared; let d = D(); a.delegate = d; a.setActivationPolicy(.regular); a.run()'
bundle AdlTwo dev.adl.check.two 'import AppKit
final class D: NSObject, NSApplicationDelegate { var w: [NSWindow] = []
 func applicationDidFinishLaunching(_ n: Notification) { for i in 0..<2 { let x = NSWindow(contentRect: NSRect(x: 120 + i * 320, y: 300, width: 280, height: 160), styleMask: [.titled], backing: .buffered, defer: false); x.title = "adl-two-\(i)"; x.orderFrontRegardless(); w.append(x) } } }
let a = NSApplication.shared; let d = D(); a.delegate = d; a.setActivationPolicy(.regular); a.run()'
osacompile -o "$W/AdlAppletQuit.app" -e 'delay 0.5' && "$LSREG" -f "$W/AdlAppletQuit.app" && REGISTERED="$REGISTERED AdlAppletQuit"
APPLET_ID=$(defaults read "$W/AdlAppletQuit.app/Contents/Info" CFBundleIdentifier)
cat > "$W/two.js" <<'JS'
ObjC.import('Cocoa');
var wins = [];
function run() { for (var i = 0; i < 2; i++) { var x = $.NSWindow.alloc.initWithContentRectStyleMaskBackingDefer($.NSMakeRect(120 + 320 * i, 520, 280, 160), $.NSWindowStyleMaskTitled, $.NSBackingStoreBuffered, false); x.title = 'adl-jxa-' + i; x.orderFrontRegardless; wins.push(x); } }
function idle() { return 30; }
JS
osacompile -l JavaScript -s -o "$W/AdlJxaTwo.app" "$W/two.js" && "$LSREG" -f "$W/AdlJxaTwo.app" && REGISTERED="$REGISTERED AdlJxaTwo"
JXA_ID=$(defaults read "$W/AdlJxaTwo.app/Contents/Info" CFBundleIdentifier)
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
two_case() { # label bundle-id app-dir-name
  local out ok win sug n focused pid
  out=$("$AD" launch "$2" --timeout 20000)
  n=0; for _ in $(seq 1 20); do
    pid=$(pgrep -f "$W/$3.app/Contents/MacOS" | head -1)
    [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null && n=$("$W/wincount" "$pid")
    [ "$n" = 2 ] && break; sleep 0.5
  done
  focused=$("$AD" list-windows --app "$3" | grep -o '"is_focused":true' | wc -l | tr -d ' ')
  echo "list-windows --app $3: $("$AD" list-windows --app "$3")"
  ok=$(printf '%s' "$out" | field ok); win=$(printf '%s' "$out" | field data.window.id); sug=$(printf '%s' "$out" | field data.suggestion)
  echo "== LIFECYCLE-12 $1 (CGWindowList layer-0 windows of pid ${pid:-none}: $n): $out"
  if [ "$n" != 2 ] || [ "$focused" != 0 ]; then echo "INVALID $1 (precondition: 2 windows, none focused; got $n windows, $focused focused)"
  elif [ "$ok" = True ] && [ -z "$win" ] && [ "$(printf '%s' "$sug" | grep -o 'w-[0-9]*' | sort -u | wc -l | tr -d ' ')" = 2 ]; then echo "PASS $1"
  else echo "FAIL $1 (expected ok:true, no data.window, data.suggestion naming 2 window ids; before the fix: AMBIGUOUS_TARGET)"; fi
  pkill -f "$W/$3.app" 2>/dev/null
}
quit_case swift-activate dev.adl.check.quit --activate
quit_case swift-background dev.adl.check.quit ""
quit_case applet-activate "$APPLET_ID" --activate
two_case swift-two dev.adl.check.two AdlTwo
two_case jxa-two "$JXA_ID" AdlJxaTwo
out=$("$AD" launch Calculator --timeout 20000); echo "== control Calculator: $out"
if [ -n "$(printf '%s' "$out" | field data.window.id)" ]; then echo "PASS control-calculator"; else echo "FAIL control-calculator (a one-window app must still report its window)"; fi
if [ "$CALC_WAS_RUNNING" = no ]; then "$AD" close-app Calculator >/dev/null 2>&1; else echo "NOTE Calculator left running (it was running before the script)"; fi
pkill -f "$W/Adl" 2>/dev/null
unregister_all
git -C "$R" worktree remove --force "$W/src"
echo "done; delete $W when finished"
```
