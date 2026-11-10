# Mac checks (batch for the Mac worker)

Owner: laptop lead. Each section names the PR, the exact head SHA the steps apply to, the steps and the expected
observations. Run a section only if the PR's current head equals the SHA written here; if it differs, this file is stale
for that PR: skip it and say so. Report per PR: head SHA, each check's PASS/FAIL line, and anything unexpected.

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
| #80 | SURFACE-10 | `2982cc00` | steps ready (below); branch 2 behind main (#14, #82), will move when re-merged |
| #58 | ACTION-12 | pending | being re-merged with main; the PR body's old steps use TextEdit and Finder, do not run them; new steps pending |
| #86 | ADAPTERS-4 + ADAPTERS-1 | pending | draft, steps pending |
| #34 | SURFACE-3 | pending | being merged up with main (after #14); privacy-constrained script pending |
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
