# State-machine audit: tasks and ledger

Branch `docs/state-machine-ledger`. Pushed at checkpoints, never opened as a PR. Per-finding detail (every
`file:line`, the states and transitions, test sketches, the exact prior-art searches) is in `audit/<AREA>.md`.

## Setup (2026-11-07, laptop)

- [x] Clone `gshost1/agent-desktop`, `upstream` remote added, `main` = `upstream/main` = `b43ac4f6`.
- [x] `gh repo set-default gshost1/agent-desktop`; `gh` logged in as gshost1 with admin/push.
- [x] Toolchain 1.89.0 (`rust-toolchain.toml`) active via `~/.cargo/bin`; `core.hooksPath = .githooks`.
      The hook needs `~/.cargo/bin` on `PATH` or it skips itself.
- [x] Writing worktrees live in `.agents/worktrees/<name>` (ignored through `.git/info/exclude`).
- [x] Baseline gates on `main`: all green, 4396 tests. One Linux test
      (`the_lifecycle_overrides_are_wired_to_the_adapter`) needs a display; the gate run starts a throwaway Xvfb.
- [ ] Linux E2E: **not runnable yet.** No logged-in desktop session for george (only the gdm greeter), which is
      fine because the harness starts private Xvfb/mutter sessions. Missing prerequisites from `runner/gate.py`:
      `xwd` (x11-apps), `xdotool`, `wmctrl`, `xclip`, `openbox`, `wl-copy`/`wl-paste` (wl-clipboard),
      `gnome-terminal`, PyQt6. Present: Xvfb, dbus, gdbus, busctl, gsettings, gnome-shell, PyGObject, Calculator,
      Text Editor, Files, Settings. Needs one `sudo apt install` from George.

## Prior art

- Upstream PR #602 (`laf/state-machine-fixes`, open, by the maintainer) fixes upstream register findings #1-#8
  (issues #597-#601). Its first commit had a 20-row register (`audit/UPSTREAM-602-register.md`); #9-#20 are open.
  The fork carries #602 as-is (PR below) instead of redoing it.
- Every row below was searched in upstream issues and PRs (open and closed), fork PRs and branches, and
  `git log upstream/main`. `upstream/main` = `main`, so nothing was dropped as already fixed. Origin has no
  `perf/` branches yet (checked 2026-11-07).

## Findings ledger

Impact: **A** wrong action delivered, **B** wrong error, recovery or disposition, **C** latency, **D** clarity.
Status: **C** confirmed by tracing the code end to end, **S** suspected (needs a probe or a live run).
Platform: where the fix lands; mac/win fixes are opened as drafts and listed under "Needs ... verification".

| Rank | ID | Machine | Main files | Illegal transition reachable today | Hidden cost | Imp | St | Prior art | PR |
|---|---|---|---|---|---|---|---|---|---|
| 1 | ACTION-2 | `type --interval-ms` burst: Landing → Burst → Typed | core `commands/type_text_interval.rs:93` | After every keystroke lands, `--wait-for` reloads the refmap; a pruned or drilled snapshot returns SNAPSHOT_NOT_FOUND/STALE_REF stamped `not_delivered` + `refresh_snapshot_then_retry_original`, so a caller types the text twice | 3 loads of one entry per command | A | C | none (#348 introduced, #362 closed) | [#2](https://github.com/gshost1/agent-desktop/pull/2) |
| 2 | STATE-9 | Interaction lease adoption: Free / Held(OFD) | core `storage/file_lock.rs:54-89`, `src/mcp/call.rs:29-35` | `mcp` started by a lease-holding harness passes the lease fd+env to up to 8 concurrent children; each "adopts" the same open file description, so mutations interleave | – | A | C mech, S reach | #160 (closed, different), PR #590 (open, lock dir only) | in progress (w4) |
| 3 | SURFACE-3 | Notification identity across CLI calls | core `notifications/`, all 3 adapters | dismiss/act address a list index; the list shifts between calls and the wrong same-app notification is dismissed or acted on | – | A | C | none | planned (M, JSON contract) |
| 4 | SURFACE-4 | Windows dismiss verification | windows `notifications/` | with an identical twin present, a real dismissal reports ACTION_FAILED + retry, and the retry removes the twin | – | A | C | none | planned (win draft) |
| 5 | SURFACE-10 | macOS `--surface focused` ref re-rooting | macos `tree/` surface roots | focused/menu refs re-root at whatever window or menu is current, ignoring `source_window_id` | – | A | C path, S identity catch | PR #258 fixed sheet/popover/alert only | planned (mac draft) |
| 6 | LIFECYCLE-9 | Linux remembered pre-maximize place | linux `system/window/` | after the user un-maximizes, `restore` (or minimize+restore) moves the window back to a stale saved place | – | A (low) | C | none | planned |
| 7 | ADAPTERS-2 | macOS click chain `DeliveredWithoutEffect` | macos `actions/` chain | a row that selects after the 300 ms check is treated as "press did nothing"; the chain then runs AXOpen or child presses (click + open) | – | A | S | #206 introduced; #455 (open) menu items only | probe on Mac first |
| 8 | ADAPTERS-3 | Linux `write_ignored` | linux `actions/write/text_write.rs:78-90` | "not delivered" judged from one immediate read; a bridge that applies writes late lets a retried `type` insert twice | – | A | S | none | probe (Java ATK) first |
| 9 | BINARY-12 | MCP cancel during held input | `src/mcp/calls.rs`, mac/win release guards | SIGTERM/TerminateProcess skips Drag/KeyReleaseGuard drops on mac/win; a button or key can stay down | – | A | S | #599, PR #602, #525 (cancel only) | probe on Mac first |
| 10 | ACTION-1 | Pre-dispatch phase of a ref action | core `ref_action/ref_action_wait.rs:97-106`, `ref_action_single.rs:29-52`, `actions/headed_focus.rs:14` | lease timeout, post-lease deadline, headed focus failure and strict-trace failure before dispatch report `delivery: unknown`, `retry: unknown`, no recovery, and auto-wait stops early | – | B | C | #598 / PR #602 (preflight only) | in progress (w3) |
| 11 | REFS-2 | Capture verdict (`unique_at_capture: bool`) | core `refs/`, `tree/` capture proof | a capture that ran out of time or left a field unread fails later as AMBIGUOUS_TARGET `identity_not_unique`, retryable false; right advice is "retake" | – | B | C | none (#442 introduced) | in progress (w5) |
| 12 | SURFACE-1 | Closed-menu wait phase | core `commands/wait*`, linux surface wait, mac/win menu wait | mac/win accept the first "closed" read, so `wait --menu-closed` right after the opener returns before the menu appears; Linux guard (bool + 2 s file stamp) misses raw `mouse-click`/`drag` and holds 2 s after an item click | up to 2 s per wait on Linux | B | C code, S mac/win timing | PR #291 (Linux-only fix) | planned (M) |
| 13 | REFS-4 | Resolve error triple (code, complete, retryable) | core `ref_action/ref_action_poll.rs`, `refs/` | pre-lease poll stops on STALE_REF complete=true and ignores retryable=true; a reordering list yields final `saved_ref_absent` for a live element | – | B | C path, S rate | none | planned |
| 14 | REFS-5 / ACTION-8 | Persist policy under an inherited deadline (#10) | core `snapshot/mod.rs` save, `commands/wait_selector.rs:301` | in batch, a complete snapshot/find/drill or a satisfied `--wait-for` fails TIMEOUT before the lock while a partial one is saved | – | B | C | register #10; #154, PRs #164, #372 related | planned |
| 15 | ACTION-3 / STATE-8 | Pre/post action artifacts (#17, #18) | core `ref_action/ref_action_wait.rs:91-106`, `commands/helpers.rs:148-161` | pre screenshot orphaned on lease failure; post screenshot taken after `--wait-for` on the spent deadline; a pair per auto-wait refusal | up to ~1.9 s of action budget per refusal | B | C | none (#91 introduced) | planned |
| 16 | SURFACE-2 / ACTION-4 | Notification wait baseline (#19) | core `commands/wait_notification.rs` | counts never go down (dismissed-then-identical missed); baseline taken after the trigger; batch seeds only `--event` | mac: ~1 s per headed poll (opens NC) | B | C | register #19; #433 same class | planned |
| 17 | BINARY-5 | Command effect (`is_mutating` overload) | `src/dispatch/disposition.rs:16-23` | `screenshot <PATH>`, `clipboard-get --out`, `trace export`, `session` failures keep `delivery: unknown` | – | B | C | none | planned |
| 18 | BINARY-1 | Oversized batch entry | `src/batch/result_entry.rs:210` | a read-only entry over the output budget returns `delivery_uncertain`, `retry: unsafe` | – | B | C | none | planned |
| 19 | BINARY-2 | Batch run state (baseline adjacency, stop reason) | `src/batch/execution.rs` | `[click, screenshot, wait --event]` takes the baseline after the click; `--stop-on-error` relabels a deadline stop | – | B | C | #177 / PR #182 near | planned |
| 20 | ADAPTERS-5 | Linux X11 release guard | linux `input/` X11 guard | `armed` never clears when the last key is up; a timeout between characters reports `emergency_release_posted: true, acknowledged: false` with nothing held | – | B | C | none | planned |
| 21 | ADAPTERS-4 | macOS chain step writes then fails a read | macos `actions/` DeliveryTracker | `select_containing_item` writes AXSelected, read-back times out, reported `not_delivered` | – | B | C | none | planned (mac draft) |
| 22 | ADAPTERS-1 | macOS right-click chain | macos `chain_defs.rs:83`, `chain_menu_steps.rs:137-140` | continues after an unverified AXShowMenu; a late menu makes the chain end "All chain steps exhausted" with the menu on screen; probe failure read as "no menu" | – | B (maybe A) | C | PR #399 (open, headed only) | planned (mac draft) |
| 23 | LIFECYCLE-1 | Windows launch: launcher exited / handed off (#552) | windows `system/` launch | stub pid exits, window poll runs to the deadline, reports ok with the dead pid | full `--timeout` | B | C | #552, #555, PR #483 | planned (win draft) |
| 24 | LIFECYCLE-2 | macOS launch exit watch | macos `system/` launch | an app that quits after startup "Finished" returns ok with its dead pid | – | B | C | none | planned (mac draft) |
| 25 | LIFECYCLE-5 | close-app outcome (#581) | core `commands/close_app.rs`, mac/win close | a quit prompt ends as generic TIMEOUT "app busy" on mac/win; outcome is a bool | 5 s | B | C | #581, PR #582 (open, mac, probe at deadline) | build on #582 |
| 26 | LIFECYCLE-6 | close-app single 5 s budget | core `commands/close_app.rs` | lease wait spends the exit-check time → TIMEOUT/unsafe; macOS app exiting mid-close → APP_UNRESPONSIVE | – | B | C | none | planned |
| 27 | LIFECYCLE-3 | launch lease scope | core `commands/launch.rs`, adapters | launch holds the machine-wide lease through the window wait and CDP probe; other agents TIMEOUT for up to 30 s | up to 30 s blocked for others | B/C | C | PR #436 (open) leaves it | planned |
| 28 | LIFECYCLE-7 | Chromium/Electron AX activation | core + mac/win activation | win read-only snapshot needs the lease for a no-op 50 ms sleep; mac sleeps 2 s holding the lease; activation can overrun the command timeout | 50 ms (win), 2 s (mac) under lease | B/C | C (7b-d), S (7a) | #465, #254, #238 | planned |
| 29 | LIFECYCLE-8 | Linux window settle | linux `system/window/` | fixed 250 ms limit vs 120 ms confirm pause: a placement landing after ~130 ms always reports `readback_mismatch` | 250 ms | B | C logic, S frequency | none | planned |
| 30 | LIFECYCLE-10 | focus-window double confirmation | core `commands/focus_window.rs` | core's second check has no reserved time; an adapter-confirmed focus returns ACTION_FAILED | ≥50 ms + 2 reads every call | B/C | C narrow | none | planned |
| 31 | REFS-3 | Snapshot id is mutable (drill rewrite) | core `refs/refs_store.rs:70-87`, `snapshot/snapshot_ref.rs` | a drill removes refs another agent holds → plain STALE_REF on a live element; two drills of one root erase each other | – | B | C | none | planned (M/L) |
| 32 | REFS-1 | latest pointer vs snapshot dir (#537) | core `refs/refs_store.rs`, `commands/status.rs` | `status` fails SNAPSHOT_NOT_FOUND when the pointer names a deleted snapshot; legacy migration unreachable | – | B | C | #537; PRs #538, #576, #588, #500 open | skip: open upstream PRs |
| 33 | REFS-7 | Snapshot retention 128→96 | core `refs/refs_store_prune.rs` | heavy `find` evicts a snapshot another agent acts on; error blames session scoping | – | B (low) | C | PR #389 (open, TTL) | planned (S) |
| 34 | STATE-1 | FFI last-error (#9) | ffi `error.rs:84-91,176-185` | failure during thread teardown returns AD_RESULT_OK | – | B | C | none | planned |
| 35 | STATE-4 / BINARY-9 | `mcp --debug --screenshot`, `mcp --wait-for` (#11) | `src/mcp/`, `src/visual_debug/mod.rs:56-65` | per-command globals go to every tool call; non-snapshot tools fail from the first call, snapshot/click after the first | – | B | C | none | planned |
| 36 | STATE-5 / BINARY-10 | `session start --cursor` two-step write (#12) | `src/dispatch/session.rs:159-221` | second write fails → error has no session id; session stays active | – | B | C | none | planned |
| 37 | STATE-6 | Manifest-less session dirs (#15) | core `session/gc.rs` | invisible to GC; one failed delete aborts the GC loop | disk | B | C | #601, PR #498 (open) | build on #498 |
| 38 | STATE-7 | Trace reader command pairing (#16) | core `trace/trace_read/merge.rs:95-155` | concurrent same-named FFI commands overwrite each other's start; real lost end hidden | – | B | C | none | planned |
| 39 | STATE-12 / BINARY-11 | Overlay Hide/Show as a switch (#13) | `src/dispatch/mod.rs:52-68`, renderers | A's Show lands during B's headed action; mac Hide/Show take the start lock for up to 4 s | up to 4 s (mac) | B (visual) | C | #178, PR #183, PR #528 | planned (intent to confirm) |
| 40 | STATE-13 | Overlay renderer per routing mode | core `cursor_overlay/`, renderers | switching single/multi-agent leaves the old renderer; next Show draws a ghost cursor | – | B (visual) | C | #175, PR #180 | planned |
| 41 | STATE-14 | macOS overlay socket name | macos overlay `endpoint.rs:6-49` | legacy hash of `Xv2` equals current hash of `X`; `session end Xv2` errors | – | B | C | none | planned (mac draft) |
| 42 | STATE-15 | Renderer dead or unspawned | core `cursor_overlay/`, renderers | mac returns Ok without spawning when the binary is renamed (rendered: true, nothing drawn); Linux windows survive `session gc` | 0.9-4 s startup stall | B | C | #385, #422, PRs #396, #456 | planned |
| 43 | STATE-16 | Batch items' overlay config frozen at prepare | `src/batch/`, core context | `[cursor-overlay disable, click]` respawns the renderer (win/linux) | – | B (visual) | C | none | planned |
| 44 | STATE-10 | Trace write ledger | core `trace/trace_write_budget.rs` | 50 ms lock + fsync per event; under contention events dropped silently, strict mode turns a delivered action into an error | ~10-80 ms/command (est.) | B/C | C mech, S rate | #600 | planned |
| 45 | BINARY-3 | Batch ended-session gate | `src/batch/execution.rs:47-59` | `session end` on an ended session works standalone, skipped in batch | – | B (low) | C | #177 / PR #182 | planned |
| 46 | BINARY-4 | Strict trace at batch `command.end` | `src/dispatch/mod.rs:141` | a trace failure discards every per-entry result | – | B | C | none | planned |
| 47 | BINARY-6 | `settle` infers "sent nothing" from INVALID_ARGS | `src/dispatch/disposition.rs` | `press --headed` changed focus, adapter refuses → stamped `not_delivered`/safe | – | B | S | none | planned |
| 48 | BINARY-7 | `pre_dispatch_error` → INTERNAL | `src/dispatch/` | `Io(PermissionDenied)` before dispatch loses PERM_DENIED | – | B (low) | C map, S reach | PR #411 related | planned |
| 49 | SURFACE-5 | Notification session cleanup | all 3 adapters | cleanup failure replaces a successful mutation's result, delivery unknown → retry | – | B | C code, S reach | none | planned |
| 50 | SURFACE-6 | macOS notification action | macos `notifications/` | reports success right after AXPress, no re-read | – | B | C | none | planned (mac draft) |
| 51 | SURFACE-7 | macOS menu-open probe | macos surface search | 2048-node whole-app BFS; large app → APP_UNRESPONSIVE retryable false; missing AXVisible = visible | full BFS per poll | B | C code, S counts | none | planned (mac draft) |
| 52 | SURFACE-8 | menubar folded into `menu` (#512) | role vocabulary, `scripts/jev` | jev reads a menubar as an open menu | – | B | C | #512, PRs #549, #517 (open) | skip: open upstream PRs |
| 53 | SURFACE-9 | Surface signal ids reused | core `wait/signals.rs`, adapters | swapping or reopening a menu fires no `surface-appeared` | – | B | C code, S per app | none | planned |
| 54 | ACTION-5 | wait mode by Option count (#605) | `src/cli_args/`, core `commands/wait.rs` | orphan `--app`/`--timeout` pass silently | – | B | C | #605, PR #606 (open, partial) | build on #606 |
| 55 | ADAPTERS-6 | Wayland kept RemoteDesktop session | linux `input/` wayland | closed Mutter session → "delivery uncertain", no fresh-session retry | 150 ms drain per key call; 210 ms per click | B/C | S | #408, #380, #583 related | probe first |
| 56 | ADAPTERS-9 | Windows clipboard statics | windows `input/clipboard/` | `OWNER` OnceLock caches an init failure for the process (FFI); first init blocks 5 s ignoring the deadline | up to 5 s | B (FFI) | C | none | planned (win draft) |
| 57 | LIFECYCLE-12 | macOS launch AMBIGUOUS after launch | macos launch | app restoring several windows with none focused → error after a successful launch | – | B | S | none | probe first |
| 58 | STATE-17 | ProcessLeaseGuard not reentrant | core `session/process_lease_guard.rs` | same thread re-acquiring spins 5 s → TIMEOUT | 5 s | B | S | none | planned |
| 59 | STATE-18 | FFI allocation registry by address | ffi `resource.rs` | freeing a stale struct copy after address reuse frees a live allocation | – | B (misuse) | S | none | docs or L |
| 60 | BINARY-8 | Permission report fixed 5 s deadline | `src/` permissions path | ignores command/batch budget; can break the Linux "timeout + 1 s" budget | up to 5 s | C | S | none | probe first |
| 61 | LIFECYCLE-4 | Linux launch, app never joins AT-SPI | linux `system/launch_observe.rs` | waits full 30 s, then ok "not on the bus yet" | 30 s | C | C | none | planned |
| 62 | ACTION-7 | Stability evidence handoff poll → dispatch | core `ref_action/`, `actionability/stability_sampler.rs` | – | headed: 2 polls + 16 ms always; moved element 50-120 ms + 5-6 round trips | C | C | #155 | planned |
| 63 | ACTION-10 | Post-action verification settle | core `actions/post_action.rs:55-74` | toggle race reports a verified toggle that toggled nothing (S) | normalized value: 400 ms (8 reads) then ACTION_FAILED | C | C cost, S race | PR #190 | planned |
| 64 | REFS-6 | Repeated window reads, restarted deadlines | core `refs/`, `commands/get*.rs`, `wait_element.rs` | `get`/`is` start a fresh 5 s deadline (up to 10 s) | resolve looks up the window twice; `wait --element` full walk per 100 ms poll | C | C (est.) | none | measure first |
| 65 | ADAPTERS-7 | Linux headed physical → semantic → physical | linux `actions/dispatch.rs:178-181` | – | second full preflight; semantic error lost | C | C | none | planned |
| 66 | ADAPTERS-8 | Drag preflight with zero slack | windows `drag.rs`, linux `drag_plan.rs` | tight timeout: admitted drag aborts mid-gesture, emergency release at origin | – | C | C | none | planned |
| 67 | ADAPTERS-10 | Linux X11 keymap snapshot | linux `input/keyboard/` | layout replaced mid-type → other characters (S) | 2 XTest connects + 2 journal sweeps per action; 50 ms per remapped char | C | S race | none | planned |
| 68 | LIFECYCLE-11 | "Provisional placement" three ways | adapters `system/window/` | – | win always sleeps 80 ms | C/D | S | PR #393 | planned |
| 69 | STATE-11 | Session manifest read 4× per command | core `context/mod.rs`, `session/` | trace/overlay/ended decided from different manifest versions | 4 reads | C/D | C | #601, PR #496 | after #602 |
| 70 | ACTION-6 | Delivery disposition as a lattice | core `response/adapter_error.rs:50-53` + 8 sites | `trace_error_after_delivery` turns satisfied `not_delivered` into `delivered_unverified`; no enforcement of "no input before `before_first_event`" | – | D (guards A) | C | none | refactor after ACTION-1 |
| 71 | ACTION-9 | Auto-wait loop and poll state | core `ref_action/ref_action_wait.rs`, `ref_action_poll_state.rs` | metrics count the last iteration only; each refusal raises or scrolls again | repeated focus/scroll | C/D | C | none | planned |
| – | D rows | REFS-8, REFS-9, ACTION-11, BINARY-13, BINARY-14, LIFECYCLE-13/14/15, STATE-2, STATE-3, STATE-19, SURFACE-11, ADAPTERS-11..18 | see `audit/` | no reachable illegal transition found, or code clarity only | small | D | – | – | – |

## PRs (fork only)

| # | Branch | Finding | PR | Status | Upstream |
|---|---|---|---|---|---|
| 1 | `fix/carry-upstream-602-state-transitions` | upstream #1-#8 | [#1](https://github.com/gshost1/agent-desktop/pull/1) | **merged** 2026-11-07 as `c4d39615`; CI 13/13 green on head, gates green (4402), reviews done | overlaps `#602` (goes back as a note to its author, not as our PR) |
| 2 | `fix/type-interval-post-delivery-state` | ACTION-2 | [#2](https://github.com/gshost1/agent-desktop/pull/2) | open; gates green (4397), 2 tests invert-verified, code review applied | ready (when merged; no dependency on fork-only work) |
| 3 | `fix/ref-action-pre-dispatch-phase` | ACTION-1 | – | written (w3), gates green (4406); review round 1 being applied | ready (when merged; rebased on the `#602` carry, so upstream needs `#602` first or a rebase) |
| 4 | `fix/mcp-inherited-lease-exclusive` | STATE-9 | – | written (w4), gates green (4402); code + security review round 1 being applied | ready (when merged; touches `src/mcp/calls.rs`, which `#602` also changes) |
| 5 | `fix/capture-verdict-not-ambiguous` | REFS-2 | – | written (w5), gates green (4403); envelope bump 2.6 -> 2.7 in progress; will be draft (mac test literals) | needs-issue-first (changes the JSON contract: envelope 2.6 -> 2.7, AMBIGUOUS_TARGET -> STALE_REF) |

## Needs Mac verification

## Needs Windows verification

## Blocked on George

- Linux E2E prerequisites: `sudo -n apt-get install ...` was denied by this session's Claude Code permission layer
  before sudo ran. Run on the laptop: `sudo apt-get install -y x11-apps xdotool wmctrl xclip openbox wl-clipboard
  gnome-terminal python3-pyqt6`, or allow it for the session.
- Fork CI: `pull_request` runs do not start on fork PRs; CI is dispatched with `workflow_dispatch` on the head SHA
  (counts for merging per George). CodeQL and Supply Chain are `disabled_fork`.

## Upstream policy (George, 2026-11-07)

Nothing goes to vercel-labs yet: no PRs, issues, comments, or links that create timeline events. Every fix stays
upstream-ready: one finding per branch cut from `main`, no dependency on fork-only work (browser crate, perf
branches), upstream references as inline code, PR prose via the technical-writer skill. The Upstream column is
`ready`, `needs-issue-first` (JSON contract change or large refactor), `overlaps #N`, or `fork-only`.

## Merge rule (George, 2026-11-07)

Merge own fork PRs when: every CI check on the head SHA is green (dispatch runs count), local gates and reviews are
done, not a draft, rebased on current `origin/main` and re-tested. `gh pr merge <n> --squash --delete-branch`.
A PR that touches root `Cargo.toml`: first confirm `gh workflow list --repo gshost1/agent-desktop --all` shows
Release `disabled_manually` (a push to `main` touching it would cut a fork release); otherwise hold and tell the
control thread.

## Incidents

- PR #1's first body (2026-11-07) mentioned the upstream author with a bare @-handle and linked upstream PR 602 by
  URL. Fixed within a minute, but GitHub already notified the author and recorded a cross-reference on upstream
  #602's timeline; that event cannot be removed from the fork. All later PR bodies reference upstream items in
  inline code only (no autolink, no backlink).

## Found (outside scope)

- `commands::wait_selector::tests::retry_tests::gone_wait_requires_window_absence_after_accessibility_disappears`
  (250 ms budget) failed once on the macOS CI runner for PR #2 with an unrelated diff; re-run requested. Likely a
  slow-runner timing flake.

- `crates/linux/src/system/overlay/cursor_overlay_tests.rs:31-34` binds a Unix socket under the crate directory;
  from a checkout path longer than ~80 characters it exceeds the 108-byte socket path limit and fails. Worktrees
  here use short names (`w1`...) to avoid it.
- `agent-desktop-linux adapter::tests::the_lifecycle_overrides_are_wired_to_the_adapter` needs a display; the
  pre-commit hook fails without one. Commits here run the hook under a throwaway Xvfb.
- `snapshot/mod.rs:202` sets `data.identity_unproven` to a reason string; docs and skills say `true` (from REFS-2).
- The Linux clipboard owner and overlay renderers can inherit the lease fd from a child that never adopts it, so
  the lock can outlive the parent (from STATE-9); belongs in core startup.

- Upstream has many open PRs in areas this audit touches; rows above say "build on #N" where one covers a finding.
