# Mac checks (batch for the Mac worker)

Owner: laptop lead. Each section names the PR, the exact head SHA the steps apply to, the steps and the expected
observations. Run a section only if the PR's current head equals the SHA written here; if it differs, this file is stale
for that PR: skip it and say so. Report per PR: head SHA, each check's PASS/FAIL line, and anything unexpected.

Rules learned from the #58 run (2026-11-10), binding for every section:
- Build from a detached worktree under `/tmp` at the SHA (`git -C ~/Projects/agent-desktop worktree add --detach /tmp/<name> <SHA>`); never `git switch` in `~/Projects/agent-desktop`. Wait `while pgrep -x cargo >/dev/null; do sleep 5; done` before building.
- Start with the desktop-space reset (`bench-tools/desktop_space.sh` from `origin/chore/speed-1.4-tasks`; its paths are Mac-local) and wait for the app or fixture window to exist and be front before the first check. A full-screen T3 Code Space made the fixture vanish after ~12 s.
- `find` on a shipped binary omits `bounds` when the match has a ref (`crates/core/src/commands/find_response.rs:61-70`); read bounds with `get <ref> --property bounds`.
- Print full error envelopes, never truncated.

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
| #88 | LIFECYCLE-2 + LIFECYCLE-12 | pending | DIRTY; being re-merged; steps pending |
| #34 | SURFACE-3 | `e3818161` (old) | HELD: control asked for 5 changes (temp AGENT_DESKTOP_HOME, catch-all, --app on action, stop on first FAIL, close the NC panel) and George's go-ahead; do not run the script below |
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

## #34 SURFACE-3: notification keys (HELD: control reviews this script before it runs)

Head: `e3818161e6201c24989de964ad567aecb0056d95` (CI was still running when this was written; check the PR head equals
this SHA and CI is green before running).

Key format after review: `<16hex>-<n>of<m>` (m = identical twins listed). A key whose twin count dropped below m is
refused; twins are grouped by full app/title/body, so a crafted digest collision is refused instead of acted on.

Privacy design:
- Sender: `osascript display notification` (macOS attributes it to Script Editor). Ownership is a random nonce
  `adbench-<12 hex>` in both title and body; an entry is "ours" only when its title is one of the three nonce titles
  AND its body contains the nonce.
- agent-desktop stdout stays in memory (json.loads, then the filter); stderr is discarded; nothing is written to disk.
- Foreign entries: only a count and sha256[:12] of their sorted key stems are printed.
- Error results print only `error.code` and `disposition.delivery`, never the message.
- Every mutation passes the bench key, `--expected-title <bench nonce title>` and `--app <bench entry's app_name>`. A
  stale-index call runs only when that index currently holds a bench entry. No dismiss-all. Cleanup dismisses bench
  keys only. Foreign key stems before vs after: any missing = FAIL.
- Caveat: `--headed` list/dismiss open the Notification Center panel, so real notifications are VISIBLE on the display
  during the run (never printed). Don't screen-record; nobody needs to watch.

Laptop lead's review notes:
- No print path reaches foreign title/body/app_name: `run()` reduces every envelope to ok/code/delivery or `data`;
  `bench_view()` keeps only bench entries' fields and foreign key stems; `show()` prints bench keys only.
- Residual risk 1: an unexpected Python exception prints a traceback (code lines and exception text; no notification
  fields are interpolated, but it isn't impossible). Optional hardening: wrap `main()` in `try/except Exception as e:
  print("ERROR", type(e).__name__); sys.exit(1)`.
- Minor: the stale-index `notification-action` call passes `--key` and `--expected-title` but not `--app` (the dismiss calls pass all three); its index is only used when it holds a bench entry.
- Residual risk 2: the keyed dismiss without an index (`dismiss(oldest)`, cleanup) relies on the PR's key resolution;
  the `--expected-title` + `--app` guards are what protect foreign entries if that code were wrong, and the
  before/after foreign stem comparison detects it.

Prereqs: System Settings > Notifications > Script Editor: Allow, style Banners, grouping Off (a collapsed stack reads
as INCONCLUSIVE). Don't touch notifications during the run (about 3-4 min).

Save the script below as `/tmp/adbench-34-check.py`, then:

```bash
export CARGO_BUILD_JOBS=2
D=/tmp/adbench-34 && rm -rf $D && git clone -q https://github.com/gshost1/agent-desktop.git $D && cd $D && git checkout -q e3818161e6201c24989de964ad567aecb0056d95 && cargo build --release -p agent-desktop && AD=$D/target/release/agent-desktop python3 -I /tmp/adbench-34-check.py; echo "exit $?"
```

Expected: every line PASS, ending `RESULT PASS`, exit 0. Exit 1 = FAIL; exit 3 = INCONCLUSIVE (permission, grouping,
or the list did not shift). Known limits: which of two identical twins closed is not observable (counts and
renumbering are checked); a foreign notification its app withdraws during the run reads as missing and FAILs (rerun).
Cleanup after: `rm -rf /tmp/adbench-34 /tmp/adbench-34-check.py`.

```python
#!/usr/bin/env python3
"""Mac check for agent-desktop notification keys (fork PR #34).

Posts its own notifications with a random nonce in the title and body and acts
only on entries that carry that nonce. Raw agent-desktop output stays in memory
and goes straight through bench_view(); nothing about another notification is
printed or written anywhere except a count and a hash of its opaque key stems.

Usage: AD=/path/to/agent-desktop python3 adbench_notification_keys.py
Exit: 0 pass, 1 fail, 3 inconclusive (environment did not allow the check).
"""
import hashlib
import json
import os
import secrets
import subprocess
import sys
import time

AD = os.environ.get("AD", "agent-desktop")
NONCE = "adbench-" + secrets.token_hex(6)
TWIN = (NONCE + "-twin", "twin body " + NONCE)
SOLO = (NONCE + "-solo", "solo body " + NONCE)
NEWER = (NONCE + "-newer", "newer body " + NONCE)
KINDS = {TWIN[0]: "twin", SOLO[0]: "solo", NEWER[0]: "newer"}
FAILS = []
INCONCLUSIVE = []


def say(line):
    print(line, flush=True)


def check(ok, label):
    say(("PASS " if ok else "FAIL ") + label)
    if not ok:
        FAILS.append(label)
    return ok


def run(*args):
    """Runs agent-desktop and returns only a sanitized summary of the envelope."""
    try:
        proc = subprocess.run(
            [AD, "--headed", *args], capture_output=True, text=True, timeout=180
        )
    except subprocess.TimeoutExpired:
        return {"ok": False, "code": "SCRIPT_TIMEOUT"}
    try:
        envelope = json.loads(proc.stdout)
    except ValueError:
        return {"ok": False, "code": "UNPARSEABLE_OUTPUT", "exit": proc.returncode}
    if envelope.get("ok"):
        return {"ok": True, "data": envelope.get("data") or {}}
    error = envelope.get("error") or {}
    return {
        "ok": False,
        "code": error.get("code"),
        "delivery": (error.get("disposition") or {}).get("delivery"),
    }


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


def is_bench(entry):
    return entry.get("title") in KINDS and NONCE in (entry.get("body") or "")


def keys_hash(keys):
    return hashlib.sha256("\n".join(sorted(keys)).encode()).hexdigest()[:12]


def bench_view():
    """Lists once; returns the bench's own entries and only counts for the rest."""
    result = run("list-notifications")
    if not result["ok"]:
        return None, result
    ours, foreign = [], []
    for entry in result["data"].get("notifications") or []:
        if is_bench(entry):
            ours.append(
                {
                    "index": entry.get("index"),
                    "key": entry.get("key"),
                    "kind": KINDS[entry["title"]],
                    "title": entry["title"],
                    "app": entry.get("app_name"),
                }
            )
        else:
            foreign.append(stem(entry.get("key")))
    return {"ours": ours, "foreign": foreign}, result


def kind(view, name):
    return sorted((e for e in view["ours"] if e["kind"] == name), key=lambda e: e["index"])


def show(view, label):
    shown = ", ".join(f"{e['kind']}@{e['index']}={e['key']}" for e in sorted(view["ours"], key=lambda e: e["index"]))
    say(f"     {label}: bench [{shown}]; {len(view['foreign'])} other present (keys hash {keys_hash(view['foreign'])})")


def post(pair):
    subprocess.run(
        [
            "osascript",
            "-e", "on run argv",
            "-e", "display notification (item 2 of argv) with title (item 1 of argv)",
            "-e", "end run",
            pair[0],
            pair[1],
        ],
        check=True,
        capture_output=True,
    )


def wait_for(predicate, label, timeout_s=40):
    deadline = time.monotonic() + timeout_s
    last = None
    while time.monotonic() < deadline:
        view, result = bench_view()
        if view is None:
            last = result.get("code")
        elif predicate(view):
            return view
        time.sleep(2)
    say(f"INCONCLUSIVE {label} did not appear within {timeout_s}s (last list error: {last})")
    INCONCLUSIVE.append(label)
    return None


def refusal(result):
    return (not result["ok"]) and result.get("code") == "NOTIFICATION_NOT_FOUND" and result.get("delivery") == "not_delivered"


def outcome(result):
    if result["ok"]:
        return "ok"
    return f"{result.get('code')} / {result.get('delivery')}"


def dismiss(entry, index=None):
    args = ["dismiss-notification"]
    if index is not None:
        args.append(str(index))
    args += ["--key", entry["key"], "--expected-title", entry["title"]]
    if entry.get("app"):
        args += ["--app", entry["app"]]
    return run(*args)


def cleanup():
    for _ in range(4):
        view, _ = bench_view()
        if view is None or not view["ours"]:
            return view
        for entry in view["ours"]:
            dismiss(entry)
    view, _ = bench_view()
    return view


def scenario(baseline):
    post(TWIN)
    time.sleep(1.5)
    post(TWIN)
    first = wait_for(lambda v: len(kind(v, "twin")) >= 2, "two identical bench notifications")
    if first is None:
        return
    show(first, "after two twins")
    twins = kind(first, "twin")
    if not check(len(twins) == 2, f"exactly two twins listed (saw {len(twins)}; >2 means the center keeps duplicates)"):
        return
    parsed = parse_key(twins[0]["key"])
    if not check(parsed is not None, f"keys have the digest-NofM form ({twins[0]['key']})"):
        return
    digest = parsed[0]
    check({e["key"] for e in twins} == {f"{digest}-1of2", f"{digest}-2of2"}, "twins share one digest and are keyed 1of2 and 2of2")
    oldest = next((e for e in twins if e["key"] == f"{digest}-1of2"), None)
    if not check(oldest is not None, "the 1of2 twin is listed"):
        return
    check(oldest["index"] == max(e["index"] for e in twins), "the 1of2 twin is the lower (older) entry")

    post(TWIN)
    second = wait_for(lambda v: len(kind(v, "twin")) >= 3, "third identical bench notification")
    if second is None:
        return
    show(second, "after third twin")
    twins3 = kind(second, "twin")
    keys3 = {e["key"] for e in twins3}
    check(keys3 == {f"{digest}-{n}of3" for n in (1, 2, 3)}, "the identical newcomer took 3of3; the older twins kept their numbers")
    newest = next((e for e in twins3 if e["key"] == f"{digest}-3of3"), None)
    check(newest is not None and newest["index"] == min(e["index"] for e in twins3), "the 3of3 twin is listed on top (newest first)")
    oldest_now = next((e for e in twins3 if e["key"] == f"{digest}-1of3"), None)
    shifted = oldest_now is not None and oldest_now["index"] != oldest["index"]
    check(shifted, f"the oldest twin moved from index {oldest['index']} to {oldest_now and oldest_now['index']}")
    second_oldest = next((e for e in twins3 if e["key"] == f"{digest}-2of3"), None)

    occupant = next((e for e in second["ours"] if e["index"] == oldest["index"]), None)
    if shifted and occupant is not None:
        stale = dismiss(oldest, index=oldest["index"])
        check(refusal(stale), f"stale index {oldest['index']} with key {oldest['key']} is refused NOTIFICATION_NOT_FOUND/not_delivered (got {outcome(stale)})")
        after_stale, _ = bench_view()
        if after_stale is not None:
            check(len(kind(after_stale, "twin")) == 3, "the refused dismiss closed no twin (still 3)")
    else:
        say("INCONCLUSIVE stale-index refusal skipped: the old index does not hold a bench entry")
        INCONCLUSIVE.append("stale twin index")

    keyed = dismiss(oldest)
    check(keyed["ok"], f"dismiss --key {oldest['key']} succeeds (got {outcome(keyed)})")
    third = wait_for(lambda v: len(kind(v, "twin")) <= 2, "twin count back to two")
    if third is None:
        return
    show(third, "after keyed dismiss")
    remaining = {e["key"] for e in kind(third, "twin")}
    check(remaining == {f"{digest}-1of2", f"{digest}-2of2"}, "two twins remain, renumbered 1of2 and 2of2 (which identical twin closed is not observable)")

    if second_oldest is not None:
        left = dismiss(second_oldest)
        check(refusal(left), f"key {second_oldest['key']} from before a twin left is refused NOTIFICATION_NOT_FOUND/not_delivered (got {outcome(left)})")
        after_left, _ = bench_view()
        if after_left is not None:
            check(len(kind(after_left, "twin")) == 2, "the refused twin-left dismiss closed no twin (still 2)")

    post(SOLO)
    with_solo = wait_for(lambda v: len(kind(v, "solo")) == 1, "distinct bench notification")
    if with_solo is None:
        return
    solo = kind(with_solo, "solo")[0]
    post(NEWER)
    with_newer = wait_for(lambda v: len(kind(v, "newer")) == 1, "newer distinct bench notification")
    if with_newer is None:
        return
    show(with_newer, "after solo and newer")
    solo_now = next((e for e in kind(with_newer, "solo") if e["key"] == solo["key"]), None)
    check(solo_now is not None, "the solo entry kept its key after a newer arrival")
    if solo_now is None:
        return
    occupant = next((e for e in with_newer["ours"] if e["index"] == solo["index"]), None)
    if solo_now["index"] != solo["index"] and occupant is not None:
        stale = dismiss(solo, index=solo["index"])
        check(refusal(stale), f"stale solo index {solo['index']} is refused NOTIFICATION_NOT_FOUND/not_delivered (got {outcome(stale)})")
        stale_action = run("notification-action", str(solo["index"]), NONCE + "-no-such-action", "--key", solo["key"], "--expected-title", solo["title"])
        check(refusal(stale_action), f"stale-index notification-action is refused NOTIFICATION_NOT_FOUND/not_delivered (got {outcome(stale_action)})")
    else:
        say("INCONCLUSIVE stale solo index skipped: the solo entry did not shift onto another bench entry")
        INCONCLUSIVE.append("stale solo index")
    exact = dismiss(solo_now, index=solo_now["index"])
    check(exact["ok"], f"dismiss {solo_now['index']} --key {solo['key']} succeeds (got {outcome(exact)})")
    final = wait_for(lambda v: not kind(v, "solo"), "solo entry gone")
    if final is None:
        return
    show(final, "after solo dismiss")
    check(len(kind(final, "newer")) == 1 and len(kind(final, "twin")) == 2, "only the solo entry closed: newer and both twins remain")


def main():
    say(f"bench nonce {NONCE}")
    baseline, result = bench_view()
    if baseline is None:
        say(f"INCONCLUSIVE list-notifications failed before posting: {outcome(result)}")
        return 3
    say(f"     before: {len(baseline['foreign'])} other notifications present (keys hash {keys_hash(baseline['foreign'])})")
    try:
        scenario(baseline)
    except subprocess.CalledProcessError:
        say("INCONCLUSIVE osascript could not post a notification")
        INCONCLUSIVE.append("post")
    finally:
        left = cleanup()
    if left is None:
        say("FAIL could not list after cleanup")
        return 1
    check(not left["ours"], f"cleanup removed every bench notification ({len(left['ours'])} left)")
    before, after = set(baseline["foreign"]), set(left["foreign"])
    missing, added = before - after, after - before
    say(f"     after: {len(after)} other notifications present (keys hash {keys_hash(after)})")
    check(not missing, f"{len(before)} other notifications untouched: {len(missing)} missing, {len(added)} arrived during the run")
    if FAILS:
        say(f"RESULT FAIL ({len(FAILS)} failed checks)")
        return 1
    if INCONCLUSIVE:
        say(f"RESULT INCONCLUSIVE ({', '.join(INCONCLUSIVE)})")
        return 3
    say("RESULT PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
```
