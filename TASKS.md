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
| 1 | ACTION-2 | `type --interval-ms` burst: Landing → Burst → Typed | core `commands/type_text_interval.rs:93` | After every keystroke lands, `--wait-for` reloads the refmap; a pruned or drilled snapshot returns SNAPSHOT_NOT_FOUND/STALE_REF stamped `not_delivered` + `refresh_snapshot_then_retry_original`, so a caller types the text twice | 3 loads of one entry per command | A | C | none (#348 introduced, #362 closed) | [#2](https://github.com/gshost1/agent-desktop/pull/2) merged |
| 2 | STATE-9 | Interaction lease adoption: Free / Held(OFD) | core `storage/file_lock.rs:54-89`, `src/mcp/call.rs:29-35` | `mcp` started by a lease-holding harness passes the lease fd+env to up to 8 concurrent children; each "adopts" the same open file description, so mutations interleave | – | A | C mech, S reach | #160 (closed, different), PR #590 (open, lock dir only) | [#13](https://github.com/gshost1/agent-desktop/pull/13) merged |
| 3 | SURFACE-3 | Notification identity across CLI calls | core `notifications/`, all 3 adapters | dismiss/act address a list index; the list shifts between calls and the wrong same-app notification is dismissed or acted on | – | A | C | none | [#34](https://github.com/gshost1/agent-desktop/pull/34) draft |
| 4 | SURFACE-4 | Windows dismiss verification | windows `notifications/` | with an identical twin present, a real dismissal reports ACTION_FAILED + retry, and the retry removes the twin | – | A | C | none | planned (win draft) |
| 5 | SURFACE-10 | macOS `--surface focused` ref re-rooting | macos `tree/` surface roots | focused/menu refs re-root at whatever window or menu is current, ignoring `source_window_id` | – | A | C path, S identity catch | PR #258 fixed sheet/popover/alert only | planned (mac draft) |
| 6 | LIFECYCLE-9 | Linux remembered pre-maximize place | linux `system/window/` | after the user un-maximizes, `restore` (or minimize+restore) moves the window back to a stale saved place | – | A (low) | C | none | [#22](https://github.com/gshost1/agent-desktop/pull/22) merged |
| 7 | ADAPTERS-2 | macOS click chain `DeliveredWithoutEffect` | macos `actions/` chain | a row that selects after the 300 ms check is treated as "press did nothing"; the chain then runs AXOpen or child presses (click + open) | – | A | S | #206 introduced; #455 (open) menu items only | probe on Mac first |
| 8 | ADAPTERS-3 | Linux `write_ignored` | linux `actions/write/text_write.rs:78-90` | "not delivered" judged from one immediate read; a bridge that applies writes late lets a retried `type` insert twice | – | A | S | none | probe (Java ATK) first |
| 9 | BINARY-12 | MCP cancel during held input | `src/mcp/calls.rs`, mac/win release guards | SIGTERM/TerminateProcess skips Drag/KeyReleaseGuard drops on mac/win; a button or key can stay down | – | A | S | #599, PR #602, #525 (cancel only) | probe on Mac first |
| 9a | MAC-SAVE-1 | Save-panel name field: field value vs saved path | macos set-value chain; core `actions/post_action.rs:176-190`, `tree/value_matches.rs:9-34` | macOS `set-value` of a full path into a Save panel name field returns `ok` (field reads back exactly the requested string, so element-scope verification passes), but the panel later saves `:private:tmp:…:x1.txt` in the default folder because macOS turns `/` into `:`. Likely the misnamed extra file in Torben's benchmark. Linux/Windows sibling (code read, not probed): core comparison is exact modulo CR, so a stored value that differs fails ACTION_FAILED rather than `ok`; GTK/Qt and the Windows common item dialog treat an absolute path in the name field as a destination path, so the same input is expected to save where asked | – | A | C (mac, reported), S (linux/win) | reported by the control thread from the Mac speed session | **owned by the Mac speed session; not fixed here** |
| 10 | ACTION-1 | Pre-dispatch phase of a ref action | core `ref_action/ref_action_wait.rs:97-106`, `ref_action_single.rs:29-52`, `actions/headed_focus.rs:14` | lease timeout, post-lease deadline, headed focus failure and strict-trace failure before dispatch report `delivery: unknown`, `retry: unknown`, no recovery, and auto-wait stops early | – | B | C | #598 / PR #602 (preflight only) | [#10](https://github.com/gshost1/agent-desktop/pull/10) merged |
| 11 | REFS-2 | Capture verdict (`unique_at_capture: bool`) | core `refs/`, `tree/` capture proof | a capture that ran out of time or left a field unread fails later as AMBIGUOUS_TARGET `identity_not_unique`, retryable false; right advice is "retake" | – | B | C | none (#442 introduced) | [#14](https://github.com/gshost1/agent-desktop/pull/14) draft |
| 12 | SURFACE-1 | Closed-menu wait phase | core `commands/wait*`, linux surface wait, mac/win menu wait | mac/win accept the first "closed" read, so `wait --menu-closed` right after the opener returns before the menu appears; Linux guard (bool + 2 s file stamp) misses raw `mouse-click`/`drag` and holds 2 s after an item click | up to 2 s per wait on Linux | B | C code, S mac/win timing | PR #291 (Linux-only fix) | [#18](https://github.com/gshost1/agent-desktop/pull/18) draft |
| 13 | REFS-4 | Resolve error triple (code, complete, retryable) | core `ref_action/ref_action_poll.rs`, `refs/` | pre-lease poll stops on STALE_REF complete=true and ignores retryable=true; a reordering list yields final `saved_ref_absent` for a live element | – | B | C path, S rate | none | [#17](https://github.com/gshost1/agent-desktop/pull/17) merged |
| 14 | REFS-5 / ACTION-8 | Persist policy under an inherited deadline (#10) | core `snapshot/mod.rs` save, `commands/wait_selector.rs:301` | in batch, a complete snapshot/find/drill or a satisfied `--wait-for` fails TIMEOUT before the lock while a partial one is saved | – | B | C | register #10; #154, PRs #164, #372 related | [#11](https://github.com/gshost1/agent-desktop/pull/11) merged |
| 15 | ACTION-3 / STATE-8 | Pre/post action artifacts (#17, #18) | core `ref_action/ref_action_wait.rs:91-106`, `commands/helpers.rs:148-161` | pre screenshot orphaned on lease failure; post screenshot taken after `--wait-for` on the spent deadline; a pair per auto-wait refusal | up to ~1.9 s of action budget per refusal | B | C | none (#91 introduced) | [#23](https://github.com/gshost1/agent-desktop/pull/23) merged |
| 16 | SURFACE-2 / ACTION-4 | Notification wait baseline (#19) | core `commands/wait_notification.rs` | counts never go down (dismissed-then-identical missed); baseline taken after the trigger; batch seeds only `--event` | mac: ~1 s per headed poll (opens NC) | B | C | register #19; #433 same class | [#16](https://github.com/gshost1/agent-desktop/pull/16) merged |
| 17 | BINARY-5 | Command effect (`is_mutating` overload) | `src/dispatch/disposition.rs:16-23` | `screenshot <PATH>`, `clipboard-get --out`, `trace export`, `session` failures keep `delivery: unknown` | – | B | C | none | [#21](https://github.com/gshost1/agent-desktop/pull/21) merged |
| 18 | BINARY-1 | Oversized batch entry | `src/batch/result_entry.rs:210` | a read-only entry over the output budget returns `delivery_uncertain`, `retry: unsafe` | – | B | C | none | [#21](https://github.com/gshost1/agent-desktop/pull/21) merged |
| 19 | BINARY-2 | Batch run state (baseline adjacency, stop reason) | `src/batch/execution.rs` | `[click, screenshot, wait --event]` takes the baseline after the click; `--stop-on-error` relabels a deadline stop | – | B | C | #177 / PR #182 near | **merged** [#42](https://github.com/gshost1/agent-desktop/pull/42) `47194fa6` |
| 20 | ADAPTERS-5 | Linux X11 release guard | linux `input/` X11 guard | `armed` never clears when the last key is up; a timeout between characters reports `emergency_release_posted: true, acknowledged: false` with nothing held | – | B | C | none | [#15](https://github.com/gshost1/agent-desktop/pull/15) merged |
| 21 | ADAPTERS-4 | macOS chain step writes then fails a read | macos `actions/` DeliveryTracker | `select_containing_item` writes AXSelected, read-back times out, reported `not_delivered` | – | B | C | none | planned (mac draft) |
| 22 | ADAPTERS-1 | macOS right-click chain | macos `chain_defs.rs:83`, `chain_menu_steps.rs:137-140` | continues after an unverified AXShowMenu; a late menu makes the chain end "All chain steps exhausted" with the menu on screen; probe failure read as "no menu" | – | B (maybe A) | C | PR #399 (open, headed only) | planned (mac draft) |
| 23 | LIFECYCLE-1 | Windows launch: launcher exited / handed off (#552) | windows `system/` launch | stub pid exits, window poll runs to the deadline, reports ok with the dead pid | full `--timeout` | B | C | #552, #555, PR #483 | planned (win draft) |
| 24 | LIFECYCLE-2 | macOS launch exit watch | macos `system/` launch | an app that quits after startup "Finished" returns ok with its dead pid | – | B | C | none | planned (mac draft) |
| 25 | LIFECYCLE-5 | close-app outcome (#581) | core `commands/close_app.rs`, mac/win close | a quit prompt ends as generic TIMEOUT "app busy" on mac/win; outcome is a bool | 5 s | B | C | #581, PR #582 (open, mac, probe at deadline) | build on #582 |
| 26 | LIFECYCLE-6 | close-app single 5 s budget | core `commands/close_app.rs` | lease wait spends the exit-check time → TIMEOUT/unsafe; macOS app exiting mid-close → APP_UNRESPONSIVE | – | B | C | none | [#38](https://github.com/gshost1/agent-desktop/pull/38) draft |
| 27 | LIFECYCLE-3 | launch lease scope | core `commands/launch.rs`, adapters | launch holds the machine-wide lease through the window wait and CDP probe; other agents TIMEOUT for up to 30 s | up to 30 s blocked for others | B/C | C | PR #436 (open) leaves it | planned |
| 28 | LIFECYCLE-7 | Chromium/Electron AX activation | core + mac/win activation | win read-only snapshot needs the lease for a no-op 50 ms sleep; mac sleeps 2 s holding the lease; activation can overrun the command timeout | 50 ms (win), 2 s (mac) under lease | B/C | C (7b-d), S (7a) | #465, #254, #238 | planned |
| 29 | LIFECYCLE-8 | Linux window settle | linux `system/window/` | fixed 250 ms limit vs 120 ms confirm pause: a placement landing after ~130 ms always reports `readback_mismatch` | 250 ms | B | C logic, S frequency | none | **merged** [#43](https://github.com/gshost1/agent-desktop/pull/43) `8806c6c2` |
| 30 | LIFECYCLE-10 | focus-window double confirmation | core `commands/focus_window.rs` | core's second check has no reserved time; an adapter-confirmed focus returns ACTION_FAILED | ≥50 ms + 2 reads every call | B/C | C narrow | none | **merged** [#46](https://github.com/gshost1/agent-desktop/pull/46) `42b97e61` |
| 31 | REFS-3 | Snapshot id is mutable (drill rewrite) | core `refs/refs_store.rs:70-87`, `snapshot/snapshot_ref.rs` | a drill removes refs another agent holds → plain STALE_REF on a live element; two drills of one root erase each other | – | B | C | none | planned (M/L) |
| 32 | REFS-1 | latest pointer vs snapshot dir (#537) | core `refs/refs_store.rs`, `commands/status.rs` | `status` fails SNAPSHOT_NOT_FOUND when the pointer names a deleted snapshot; legacy migration unreachable | – | B | C | #537; PRs #538, #576, #588, #500 open | skip: open upstream PRs |
| 33 | REFS-7 | Snapshot retention 128→96 | core `refs/refs_store_prune.rs` | heavy `find` evicts a snapshot another agent acts on; error blames session scoping | – | B (low) | C | PR #389 (open, TTL) | planned (S) |
| 34 | STATE-1 | FFI last-error (#9) | ffi `error.rs:84-91,176-185` | failure during thread teardown returns AD_RESULT_OK | – | B | C | none | **merged** [#37](https://github.com/gshost1/agent-desktop/pull/37) `5f85dcfa` |
| 35 | STATE-4 / BINARY-9 | `mcp --debug --screenshot`, `mcp --wait-for` (#11) | `src/mcp/`, `src/visual_debug/mod.rs:56-65` | per-command globals go to every tool call; non-snapshot tools fail from the first call, snapshot/click after the first | – | B | C | none | **merged** [#52](https://github.com/gshost1/agent-desktop/pull/52) `9ea5d145` |
| 36 | STATE-5 / BINARY-10 | `session start --cursor` two-step write (#12) | `src/dispatch/session.rs:159-221` | second write fails → error has no session id; session stays active | – | B | C | none | [#35](https://github.com/gshost1/agent-desktop/pull/35) merged |
| 37 | STATE-6 | Manifest-less session dirs (#15) | core `session/gc.rs` | invisible to GC; one failed delete aborts the GC loop | disk | B | C | #601, PR #498 (open) | build on #498 |
| 38 | STATE-7 | Trace reader command pairing (#16) | core `trace/trace_read/merge.rs:95-155` | concurrent same-named FFI commands overwrite each other's start; real lost end hidden | – | B | C | none | **merged** [#51](https://github.com/gshost1/agent-desktop/pull/51) `3fce1ebb` |
| 39 | STATE-12 / BINARY-11 | Overlay Hide/Show as a switch (#13) | `src/dispatch/mod.rs:52-68`, renderers | A's Show lands during B's headed action; mac Hide/Show take the start lock for up to 4 s | up to 4 s (mac) | B (visual) | C | #178, PR #183, PR #528 | planned (intent to confirm) |
| 40 | STATE-13 | Overlay renderer per routing mode | core `cursor_overlay/`, renderers | switching single/multi-agent leaves the old renderer; next Show draws a ghost cursor | – | B (visual) | C | #175, PR #180 | planned |
| 41 | STATE-14 | macOS overlay socket name | macos overlay `endpoint.rs:6-49` | legacy hash of `Xv2` equals current hash of `X`; `session end Xv2` errors | – | B | C | none | planned (mac draft) |
| 42 | STATE-15 | Renderer dead or unspawned | core `cursor_overlay/`, renderers | mac returns Ok without spawning when the binary is renamed (rendered: true, nothing drawn); Linux windows survive `session gc` | 0.9-4 s startup stall | B | C | #385, #422, PRs #396, #456 | planned |
| 43 | STATE-16 | Batch items' overlay config frozen at prepare | `src/batch/`, core context | `[cursor-overlay disable, click]` respawns the renderer (win/linux) | – | B (visual) | C | none | planned |
| 44 | STATE-10 | Trace write ledger | core `trace/trace_write_budget.rs` | 50 ms lock + fsync per event; under contention events dropped silently, strict mode turns a delivered action into an error | ~10-80 ms/command (est.) | B/C | C mech, S rate | #600 | planned |
| 45 | BINARY-3 | Batch ended-session gate | `src/batch/execution.rs:47-59` | `session end` on an ended session works standalone, skipped in batch | – | B (low) | C | #177 / PR #182 | **merged** [#42](https://github.com/gshost1/agent-desktop/pull/42) `47194fa6` |
| 46 | BINARY-4 | Strict trace at batch `command.end` | `src/dispatch/mod.rs:141` | a trace failure discards every per-entry result | – | B | C | none | [#21](https://github.com/gshost1/agent-desktop/pull/21) merged |
| 47 | BINARY-6 | `settle` infers "sent nothing" from INVALID_ARGS | `src/dispatch/disposition.rs` | `press --headed` changed focus, adapter refuses → stamped `not_delivered`/safe | – | B | S | none | [#21](https://github.com/gshost1/agent-desktop/pull/21) merged |
| 48 | BINARY-7 | `pre_dispatch_error` → INTERNAL | `src/dispatch/` | `Io(PermissionDenied)` before dispatch loses PERM_DENIED | – | B (low) | C map, S reach | PR #411 related | [#21](https://github.com/gshost1/agent-desktop/pull/21) merged |
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
| 71a | ACTION-12 | Pointer retry loop re-focus | core `commands/pointer_action.rs:~250` (`retry_leased_point_phase`, `focus_point_under_lease`, `scroll_point_target`) | headed `hover`/`drag` call `focus_entry_window` on every retry attempt, raising the window up to every 100 ms for the whole `--timeout`; ref actions now focus once per command (ACTION-1) | repeated focus round trips | B | C (review of ACTION-1) | none | [#58]() draft, parked: needs a "raise changed frontmost" signal |
| 71b | ACTION-13 | `preparatory_focus` parity outside ref actions | core `commands/press.rs`, `commands/pointer_action.rs` | headed `press` and pointer commands that fail after a successful focus call do not carry `details.preparatory_focus`, unlike ref actions | – | B (low) | C (review of ACTION-1) | none | planned |
| 71c | SURFACE-12 | macOS Notification Center empty read before windows populate | macos `notifications/` NcSession `list_entries` (`windows.is_empty()` early return) | opening Notification Center and reading before its AXWindows exist returns `Ok(vec![])`, a settled "no notifications" answer instead of a retryable read; a wait seed or record taken from it lets existing notifications look new | – | B | S (code review of SURFACE-2) | none | planned (mac draft) |
| 71d | STATE-20 | FFI native handles outlive their adapter | ffi `actions/native_handle.rs`, `adapter.rs` | `ad_adapter_destroy` leaves the adapter's native handles in the per-thread `HANDLES` map until `ad_free_handle` or thread exit, so platform objects are released after the adapter's runtime teardown (leak per destroyed adapter on long-lived threads) | – | B (low) | C (code review of STATE-1) | none | planned |
| 71e | STATE-21 | FFI nested call from a log callback | ffi `log_callback.rs`, tracing `with_dispatch` | an ad_* called from inside a host log callback panics in tracing (`RefCell already borrowed`, tracing-core dispatcher) and returns `AD_RESULT_ERR_INTERNAL` with panic noise on stderr; it also hides STATE-17 | – | B | C (found while fixing STATE-17) | none | in progress with STATE-17 (w30) |
| – | D rows | REFS-8, REFS-9, ACTION-11, BINARY-13, BINARY-14, LIFECYCLE-13/14/15, STATE-2, STATE-3, STATE-19, SURFACE-11, ADAPTERS-11..18 | see `audit/` | no reachable illegal transition found, or code clarity only | small | D | – | – | – |

## PRs (fork only)

| # | Branch | Finding | PR | Status | Upstream |
|---|---|---|---|---|---|
| 1 | `fix/carry-upstream-602-state-transitions` | upstream #1-#8 | [#1](https://github.com/gshost1/agent-desktop/pull/1) | **merged** 2026-11-07 as `c4d39615`; CI 13/13 green on head, gates green (4402), reviews done | overlaps `#602` (goes back as a note to its author, not as our PR) |
| 2 | `fix/type-interval-post-delivery-state` | ACTION-2 | [#2](https://github.com/gshost1/agent-desktop/pull/2) | **merged** 2026-11-07 as `f6a426db`; CI 13/13 green on head `de04c3a9`, gates green (4404), E2E no regression vs `main` | ready (no dependency on fork-only work) |
| 3 | `fix/ref-action-pre-dispatch-phase` | ACTION-1 | [#10](https://github.com/gshost1/agent-desktop/pull/10) | **merged** 2026-11-07 as `4e9420d0`; CI green on head `3b203a7f` (dispatch + pull_request runs), gates green (4423), 4 review rounds | ready (builds on the `#602` carry, so upstream needs `#602` first or a rebase) |
| 4 | `fix/mcp-inherited-lease-exclusive` | STATE-9 | [#13](https://github.com/gshost1/agent-desktop/pull/13) | **merged** 2026-11-07 as `1423c98c` (rules 1+2); CI 30/30 green on head `72fb3192`, gates green (4546), 4 code + 4 security reviews | ready (touches `src/mcp/calls.rs`, which `#602` also changes) |
| 5 | `fix/capture-verdict-not-ambiguous` | REFS-2 | [#14](https://github.com/gshost1/agent-desktop/pull/14) (draft) | head `99f7bea6`, gates green (4437), 2 review rounds applied; CI dispatched (its macOS job compiles the mac literals); stays draft: contract change | needs-issue-first (changes the JSON contract: envelope 2.6 -> 2.7, AMBIGUOUS_TARGET -> STALE_REF) |
| 6 | `fix/persist-policy-inherited-deadline` | REFS-5 / ACTION-8 | [#11](https://github.com/gshost1/agent-desktop/pull/11) | **merged** 2026-11-07 as `fe81b5c3`; CI 26/26 green on head `d2185abf`, gates green (4428), review applied | ready (upstream register item 10, no upstream issue) |
| 7 | `fix/x11-release-guard-armed-state` | ADAPTERS-5 | [#15](https://github.com/gshost1/agent-desktop/pull/15) | **merged** 2026-11-07 as `4acf87bc` (rules 1+2); CI 26/26 green on head `a52c0589`, gates green (4552), review applied | ready (Linux only, no fork-only dependency) |
| 8 | `fix/notification-wait-baseline` | SURFACE-2 / ACTION-4 | [#16](https://github.com/gshost1/agent-desktop/pull/16) | **merged** 2026-11-07 as `a3a1aedd` (rule 3: `main` moved by non-overlapping Linux files); CI 26/26 green on head `85b9c7c2`, gates green (4564) | ready (upstream register item 19, no issue) |
| 9 | `fix/menu-close-wait-phase` | SURFACE-1 | [#18](https://github.com/gshost1/agent-desktop/pull/18) (draft) | head `5b6d3a07` (main `a3a1aedd`), gates green (4591), review applied; CI dispatched; draft until the Mac check | ready after Mac check (supersedes upstream `#291`'s Linux-only guard) |
| 10 | `fix/resolve-error-classification` | REFS-4 | [#17](https://github.com/gshost1/agent-desktop/pull/17) | **merged** 2026-11-08 as `704722ca` (rules 1+2); CI 26/26 on head `8f3b3215`, gates green (4585), 3 review rounds | ready (no fork-only dependency) |
| 11 | `test/wait-timing-budgets` | CI flakes | [#19](https://github.com/gshost1/agent-desktop/pull/19) | **merged** 2026-11-07 as `f969adc8` (rules 1+2); CI 26/26 on head `686ad47b` | fork-only (test budgets) |
| 12 | `test/trace-temp-dir-cleanup` | /tmp leak | [#20](https://github.com/gshost1/agent-desktop/pull/20) | **merged** 2026-11-08 as `df4a9720` (rule 3); CI 30/30 on head `5430ae4e`; leftovers per core run 40 -> 0 | fork-only (test hygiene) |
| 13 | `fix/dispatch-delivery-settle` | BINARY-5/1/4/6/7 | [#21](https://github.com/gshost1/agent-desktop/pull/21) | **merged** 2026-11-08 as `dddc6839` (rules 1+2); CI 26/26 on head `b25843b0`, gates green (4603), 2 review rounds | ready |
| 14 | `fix/linux-restore-stale-place` | LIFECYCLE-9 | [#22](https://github.com/gshost1/agent-desktop/pull/22) | **merged** 2026-11-08 as `ec3add89` (rule 3); CI 26/26 on head `e65a688d`, gates green (4612), review applied | ready (Linux only) |
| 15 | `fix/notification-identity-key` | SURFACE-3 | [#34](https://github.com/gshost1/agent-desktop/pull/34) (draft) | head `82709a44` (main `89a3723e`), tests 4654/0, review applied; draft: contract change + device checks | needs-issue-first (new list field and --key flag) |
| 16 | `fix/action-artifact-pairing` | ACTION-3 / STATE-8 | [#23](https://github.com/gshost1/agent-desktop/pull/23) | **merged** 2026-11-08 as `6ec6e174` (rules 1+2); CI 13/13 on head `44a4cc48`, gates green (4639), 2 review rounds | ready (upstream register items 17 and 18) |
| 17 | `fix/session-start-atomic` | STATE-5 / BINARY-10 | [#35](https://github.com/gshost1/agent-desktop/pull/35) | **merged** 2026-11-08 as `17d95295` (rules 1+2); CI 13/13 on head `531de753`, gates green (4644), review applied | ready (upstream register item 12) |
| 18 | `fix/close-app-phase-budgets` | LIFECYCLE-6 | [#38](https://github.com/gshost1/agent-desktop/pull/38) (draft) | head `64b1c704`, tests 4645/0 (--no-fail-fast), review applied; draft: macOS edit | ready after Mac check |
| 19 | `fix/ffi-last-error-code` | STATE-1 / STATE-2 / STATE-3 (docs) | [#37](https://github.com/gshost1/agent-desktop/pull/37) | **merged** 2026-11-08 as `5f85dcfa` (rules 1+2); CI 27/27 on head `469eeb0a`, gates green (4647), review applied | ready (upstream register items 9, 14, 20) |
| 20 | `test/wait-selector-gone-budget` | CI flake | [#29](https://github.com/gshost1/agent-desktop/pull/29) | **merged** 2026-11-08 as `a7d301d7` (rules 1+2); CI 13/13 on head `17f6ece2`, gates green (4630) | fork-only (test budgets) |
| 22 | `test/load-sensitive-budgets` | CI flakes | [#39](https://github.com/gshost1/agent-desktop/pull/39) | **merged** 2026-11-08 as `921d6d22` (rule 3: `main` moved by PR #40, no overlapping files); CI 13/13 on head `25df1b15`, gates green (4647) | fork-only (test determinism) |
| 23 | `fix/session-gc-liveness` | session GC flake | [#40](https://github.com/gshost1/agent-desktop/pull/40) | **merged** 2026-11-08 as `a3183289` (rules 1+2); CI 13/13 on head `81dc4622`, gates green (4647); root cause: a child forked by another thread holds the session lock until exec | fork-only (test determinism) |
| 24 | `fix/batch-run-state` | BINARY-2 / BINARY-3 | [#42](https://github.com/gshost1/agent-desktop/pull/42) | **merged** 2026-11-08 as `47194fa6` (rules 1+2); CI 13/13 on head `4eb33e06`, gates green (4819), 3 high + 1 medium review rounds; simplified after round 3 | ready (prior art: upstream `#182` merged, `#177` closed) |
| 25 | `fix/linux-window-settle-provisional` | LIFECYCLE-8 | [#43](https://github.com/gshost1/agent-desktop/pull/43) | **merged** 2026-11-08 as `8806c6c2` (rules 1+2); CI 13/13 on head `29fe91d4`, gates green (4659), 2 review rounds + `readback_unsettled` fix | ready (Linux only; adds error cause `readback_unsettled`) |
| 26 | `test/inventory-poll-sibling-clock` | CI flake | [#45](https://github.com/gshost1/agent-desktop/pull/45) | **merged** 2026-11-08 as `24574e93` (rules 1+2); CI 13/13 on head `dcb62c28`, gates green (4647) | fork-only (test determinism) |
| 27 | `fix/focus-window-confirmation` | LIFECYCLE-10 | [#46](https://github.com/gshost1/agent-desktop/pull/46) | **merged** 2026-11-08 as `42b97e61` (rules 1+2); CI 13/13 on head `c2ef737b`, gates green (4826); re-cycled twice after Mac-session merges (#28, #24) | needs-issue-first (adds `data.focus_observed`) |
| 28 | `fix/trace-command-pairing` | STATE-7 | [#51](https://github.com/gshost1/agent-desktop/pull/51) | **merged** 2026-11-08 as `3fce1ebb` (rule 3: `main` moved by #53, test files only); CI 13/13 on head `19bc64c7`, gates green (4892); viewer test not yet in CI (workflow scope) | ready (upstream `#602` item 16) |
| 29 | `fix/mcp-forwarded-globals` | STATE-4 / BINARY-9 | [#52](https://github.com/gshost1/agent-desktop/pull/52) | **merged** 2026-11-08 as `9ea5d145` (rules 1+2); CI 13/13 on head `24c0ac10`, gates green on re-run (4900); code + security review | ready (upstream `#602` item 11; `#587` edits the same file) |
| 30 | `fix/process-lease-reentrancy` | STATE-17 / STATE-21 | [#55](https://github.com/gshost1/agent-desktop/pull/55) | open; reworked to an FFI-layer marker (`f8e2926e`, gates 4895; core/windows untouched); round 2 (medium): `ad_close_app` not covered, marker set while still waiting for the lease, test hygiene; writer applying | ready |
| 32 | `fix/batch-preflight-budget` | batch CLI flake root cause | [#57](https://github.com/gshost1/agent-desktop/pull/57) | **merged** 2026-11-08 as `47426721` (rule 3: `main` moved by Mac-session PRs #6 and #12, no overlap); CI 13/13 on head `e04ed040`, gates green (4901) | ready |
| 33 | `fix/pointer-focus-once` | ACTION-12 | [#58](https://github.com/gshost1/agent-desktop/pull/58) (draft) | **parked after two failed approaches** (George's "same fix twice" rule): caching the focus per command (never re-raises after another window covers the target) and stop-after-focus (a successful focus on an already-front window counts as a raise, so retries vanish; async raises on macOS/Windows turn transient occlusion into failure). Needs a "did the raise change the frontmost window" signal; findings in the PR body | needs design |
| 31 | `test/batch-cli-envelope-on-failure` | batch CLI flake diagnostics | [#53](https://github.com/gshost1/agent-desktop/pull/53) | **merged** 2026-11-08 as `d921b52e` (rules 1+2); CI 13/13 on head `72247de1`, gates green (4886) | fork-only (test diagnostics) |
| 21 | `test/npm-fixture-race` | CI flake | [#31](https://github.com/gshost1/agent-desktop/pull/31) | **merged** 2026-11-08 as `89a3723e` (rules 1+2); CI 13/13 on head `a690a037` | fork-only (test isolation) |

## Needs Mac verification

- PR #38 (LIFECYCLE-6, draft): `crates/macos/src/system/app/app_ops.rs:51`, `process/process_identity.rs:129-135`. On the Mac, on the branch: `cargo test` on the host set; then `agent-desktop close-app <app>` while another command holds the lease (e.g. a long `wait`) -> `delivered_verified`, not `TIMEOUT`; and an app that quits on its own during the close -> no `APP_UNRESPONSIVE`.

- PR #34 (SURFACE-3, draft): `crates/macos/src/notifications/actions.rs`, `system/adapter.rs`. On the Mac, on the branch: `cargo test` on the host set; then post two identical notifications, `agent-desktop list-notifications`, post a third identical one, and confirm `dismiss-notification --key <first key>` closes the original (newest-first order is assumed).

- PR #18 (SURFACE-1, draft): `crates/macos/src/system/{adapter,wait}.rs`. CI on `5b6d3a07` failed macOS clippy (`missing_const_for_thread_local` in the test seam, fixed in `832c091c`) and one Windows ARM64 live Start-menu test outside this PR; CI on `0747511e` then failed two timing-sensitive core tests and exposed a real macOS bug (the loop timed out before checking a satisfied wait); fixed in `42373866`: CI 26/26 green there, so the macOS code compiles and its unit tests pass. On the Mac, on the branch: `cargo test -p agent-desktop-core -p agent-desktop-macos -p agent-desktop -p agent-desktop-ffi -p agent-desktop-web`; then right-click a control, run `agent-desktop wait --menu-closed --app <app>` while the menu is open (returns only after it closes), and with no menu open (returns closed after about 1.5 s).

- PR #14 (REFS-2, draft, needs-issue-first): `crates/macos/src/tree/query/mod.rs` and two macOS test files. CI 30/30 green on `99f7bea6`, which compiled and ran them; the branch is being merged with `main` (conflicts with #10, #11, #17). On the Mac, on the branch: `cargo test -p agent-desktop-core -p agent-desktop-macos -p agent-desktop -p agent-desktop-ffi -p agent-desktop-web`; then `agent-desktop snapshot --app <a large app> --timeout-ms 200` until it reports `identity_unproven`, act on one of its refs and confirm `STALE_REF` `identity_unproven` / `capture_incomplete` with `retryable: false` (not `AMBIGUOUS_TARGET`); and confirm a ref with an identical twin still returns `AMBIGUOUS_TARGET` `identity_not_unique`.

## Needs Windows verification

- PR #34 (SURFACE-3, draft): `crates/windows/src/notifications/actions.rs`, `system/adapter.rs`. CI compiles and runs the Windows unit tests; on a Windows machine, repeat the identical-notification check from the Mac entry against the Action Center.

## Blocked on George

- Mac checks for drafts #14, #18, #34, #38 and the Windows check for #34 (commands under "Needs Mac verification" / "Needs Windows verification").
- Upstream issues for needs-issue-first PRs before anything goes upstream: #14 (envelope 2.7), #34 (new list field), #46 (`data.focus_observed`).
- CI wiring for PR #51's viewer test: the laptop's `gh` token has no `workflow` scope, so `.github/workflows/ci.yml` cannot be pushed from here. Either run `gh auth refresh -h github.com -s workflow` on the laptop, or make the change from the Mac: in the "Embedded visual viewer tests" step, `run: node --test src/visual_debug/viewer_filter.test.js crates/core/src/trace/trace_read/viewer_group.test.js`.
- Optional disk cleanup: old build dirs (`w5/target` 16G, `w9/target` 16G, main `target` 9.3G); nothing deleted without a yes.
- Upstream `#602` carries one cross-reference event from PR #1's first body (edited within a minute; the event persists).
- Resolved: Linux E2E packages installed by George (2026-11-07); `pull_request` CI runs on fork PRs again.

## Handoff (laptop → VPS), 2026-11-07: REVERSED, do not act on it

Reversed the same day: the VPS session could not start (sandbox could not be disabled from inside, `gh` not logged
in) and changed no code. The laptop session owns the work again; the branch states below are the resume point.
Pushed branches catch up with `main` by merging `origin/main` in (no force-push); the squash merge keeps `main` linear.

### Branch state

| Branch | Head | Base | State | Next step |
|---|---|---|---|---|
| `docs/state-machine-ledger` | this commit | – | ledger, `audit/<AREA>.md` evidence, `audit/tools/` | keep updating; never a PR |
| `fix/type-interval-post-delivery-state` (PR #2, ACTION-2) | `7e5b9e97` | `b43ac4f6` (old main) | fix + 2 invert-verified tests; code review applied; local gates green (4397); CI 13/13 green on head after re-running one macOS flake (`wait_selector` 250 ms test) | rebase on `origin/main` `c4d39615`, re-run gates, re-dispatch CI on the new head, compare E2E against a `main` baseline, then merge under the merge rule |
| `fix/ref-action-pre-dispatch-phase` (ACTION-1) | `fc4c7c32` | `c4d39615` | `pre_dispatch::run` boundary: lease timeout, post-lease deadline, headed focus, pre-dispatch trace and the rest of `dispatch_resolved_landing` now `not_delivered`; `preparatory_focus` only after a real focus call; marking lives in `actions/headed_focus.rs` so `point_resolve` gets it too; `with_detail` goes through `with_details`; json-output.md paragraph narrowed to covered steps. All 5 review items done. Core lib tests 1701/0 after rebase | full gates on `fc4c7c32` (last pre-rebase run had one flake, `batch::execution::tests::inherited_batch_deadline_bounds_slow_non_action_command`, 25 ms budget, passed 5/5 alone); `/code-review` round 2; open PR. Open, not fixed: `load_ref_entry` strict-trace error in `commands/helpers.rs` still `unknown` |
| `fix/mcp-inherited-lease-exclusive` (STATE-9) | `2cac4100` | `c4d39615` | `LeaseInheritance { Owned, Inherited, Unusable }`; mutating calls plus `status`/`permissions` take a FIFO ticketed slot; reads (snapshot, find, get, is, wait, screenshot, list-surfaces) run concurrently with no fd/env; cancel before queueing handled by ticket; slot passes on only after the terminate thread joins (`Calls::cancel` returns the `JoinHandle`); `protect_inherited_lease_descriptor` and `INTERACTION_LEASE_HANDLE_ENV` moved to core. All 7 review items done; gates green (4413); Windows clippy clean | `/code-review` + security review round 2 (touches fds/IPC); open PR. Accepted risks to state in the PR: on Windows only the env var enforces exclusivity (handle stays inheritable); waiters count against `MAX_CONCURRENT_CALLS`; a withheld read that needs renderer activation behaves as under contention |
| `fix/capture-verdict-not-ambiguous` (REFS-2) | `7736b3e6` | `c4d39615` | `CaptureVerdict { Unique, Ambiguous, Incomplete, Unknown }` replaces `unique_at_capture`; Incomplete/Unknown refuse as STALE_REF `identity_unproven` (`capture_incomplete` / `capture_unknown`) with refresh recovery; legacy bool loads via serde alias; envelope 2.6 -> 2.7 with `BREAKING CHANGE:` footer and all listed surfaces; one-line literal edits in ffi/windows/macos/linux tests | re-run gates (4 Linux lease tests timed out only because the E2E held `/tmp/agent-desktop-1000/interaction.lock`); FFI header was hand-edited (no cbindgen here): run `scripts/update-ffi-header.sh` / cbindgen verify; `/code-review`; open as **draft**, list under Needs Mac verification (3 macOS test literals not compiled); Upstream `needs-issue-first`. Leave `data.identity_unproven` string-vs-`true` mismatch for its own fix |

Overlap with the Mac speed session (`origin/perf/*`, checked at handoff): `perf/snapshot-outline` also touches
`docs/json-output.md` (ACTION-1, REFS-2) and `skills/agent-desktop/SKILL.md`,
`skills/agent-desktop/references/commands-observation.md` (REFS-2). Name these in the PR bodies; rebase whichever
merges second. `perf/cold-settle` and `perf/speed-cost` touch none of these branches' files.

### Next batch (core/Linux, testable on Linux)

1. REFS-5 / ACTION-8: persist policy under an inherited deadline (upstream register #10). Once a result is built,
   save with the detached lock; covers snapshot, find, drill and a satisfied `--wait-for`
   (`commands/wait_selector.rs:301`). Detail in `audit/REFS.md` and `audit/ACTION.md`.
2. ADAPTERS-5: Linux X11 release guard `armed` never clears when the last key is up; make it a typestate
   (Idle / Armed{held} / Released) and build the error after the release round trip. `audit/ADAPTERS.md`.
3. SURFACE-2 / ACTION-4: notification wait baseline (#19): counts must shrink when entries leave; one identity type;
   batch seed for notification waits. `audit/SURFACE.md`, `audit/ACTION.md`.
Then by rank: SURFACE-1 (menu wait phase, M, mac/win drafts), REFS-4, ACTION-3/STATE-8, BINARY-5, BINARY-1,
LIFECYCLE-9, STATE-1, STATE-4 (#11), STATE-5 (#12).

### Method that worked

- One worktree per branch under `.agents/worktrees/<short name>` (keep names short, see socket note). Exclude with
  `.git/info/exclude`. Writers get `audit/tools/writer-brief.md` plus the finding id; gates via
  `audit/tools/gates.sh <worktree>` (fmt, clippy, source rules, architecture, Windows core check, cargo tree,
  host-set tests under a throwaway Xvfb). At most 3 builders at once on an 8-core box (~9 GB target per worktree).
- Every PR: test fails on the bug first, invert-verify, gates, `/code-review` (and a security review for
  files/IPC/fds), technical-writer prose, upstream references in inline code only.
- `/security-review` refuses to run when the session cwd is not a git repo; a `security-reviewer` agent on
  `git diff origin/main...HEAD` in the worktree is the stand-in used here.
- Fork CI: `pull_request` runs do not start on fork PRs; dispatch `CI` with `workflow_dispatch` on the branch and read
  check runs on the head SHA (`gh api repos/gshost1/agent-desktop/commits/<sha>/check-runs`). CI includes macOS and
  Windows jobs, so it compiles the mac/win literal edits even though this box cannot.
- Run the Linux E2E only when no gate or hook run is active: the harness holds the machine-wide interaction lock
  and the Linux lease tests time out against it (`interaction_process_lock_timeout`).

### Linux E2E notes

- Packages (installed by George on the laptop 2026-11-07): `x11-apps xdotool wmctrl xclip openbox wl-clipboard
  gnome-terminal python3-pyqt6`, plus Xvfb, dbus, gdbus, busctl, gsettings, gnome-shell, PyGObject, GNOME
  Calculator, Text Editor, Files, Settings already present. The VPS needs the same set (`tests/e2e-linux/runner/gate.py`
  is authoritative).
- Run: `bash tests/e2e-linux/run.sh --tier x11-openbox` (133 legs, ~15 min wall on the laptop incl. release build).
- PR #2 head `7e5b9e97` on the laptop: 93 pass, 55 fail, 1 error (`audit/E2E-pr2-openbox-failures.txt`). The failures
  are in leg setup across Text Editor (no textfield ref), Calculator (`menubutton/Main Menu` missing), Settings rows,
  Terminal (`gnome-terminal-server` absent / APP_NOT_FOUND) and openbox size hints (`readback_mismatch` on
  resize). None exercises `type --interval-ms`. **No `main` baseline was run**, so "pre-existing" is unproven:
  run the same tier on `main` first and diff the failure lists before using E2E as a merge signal.
- Socket path limit: `crates/linux/src/system/overlay/cursor_overlay_tests.rs:31-34` binds a Unix socket under the
  crate dir; a checkout path over ~80 chars exceeds the 108-byte limit and the test fails. Use short worktree paths.
- `agent-desktop-linux adapter::tests::the_lifecycle_overrides_are_wired_to_the_adapter` needs a display; run tests
  and the pre-commit hook under a throwaway Xvfb (`Xvfb :9N -screen 0 1280x800x24 &` then `DISPLAY=:9N`, kill after).
- The pre-commit hook skips itself when `cargo` is not on `PATH`; export `~/.cargo/bin` first.

### Control thread and permissions

- From the laptop, `t3_thread_send` to the Mac control thread failed every time with `capability_denied` ("started
  from T3 Code · MacBook Air (36) through a linked environment, so it cannot use this environment's links"). The VPS
  link is full-access, which should not have this limit.
- On the laptop, the Claude Code permission layer denied `sudo -n apt-get ...` and an `rm -rf` of a build dir.
- T3 `t3_project_create` was denied (needs a full-access thread), so writers ran as native Agent subagents in
  shell-created worktrees instead of `delegate_task` worktrees.

### Accidental upstream ping

PR #1's first body (2026-11-07) mentioned the upstream author with a bare @-handle and linked upstream PR 602 by URL.
Both were removed within a minute, but GitHub had already notified the author and recorded a cross-reference on
upstream `#602`'s timeline; that event cannot be removed from the fork side. Upstream policy now: nothing goes to
vercel-labs, references as inline code only.

## Remaining work plan (2026-11-08)

Every open row above code clarity has one disposition:

- **Wave 1 (writers running):** trace-test `/tmp` leak (own PR); BINARY-5/1/4/6/7 (dispatch delivery settle, one
  machine); LIFECYCLE-9 (Linux stale restore, A).
- **Next waves, fixable on this laptop (core, binary, Linux, FFI):** SURFACE-3 (A, contract change: draft,
  needs-issue-first); ACTION-3/STATE-8; BINARY-2; BINARY-3; LIFECYCLE-6; LIFECYCLE-3; LIFECYCLE-7 (core part);
  LIFECYCLE-8; LIFECYCLE-10; REFS-3; REFS-7; STATE-1; STATE-4/BINARY-9; STATE-5/BINARY-10; STATE-7;
  STATE-12/BINARY-11 + STATE-13 + STATE-16 (overlay show/hide, one machine); STATE-15; STATE-10; STATE-17;
  SURFACE-5; SURFACE-9; ACTION-12; ACTION-13; LIFECYCLE-4; ACTION-7; ACTION-10; ADAPTERS-7; ADAPTERS-8;
  ADAPTERS-10; LIFECYCLE-11; ACTION-9.
- **Mac/Windows drafts (written here, compiled and unit-tested by CI, verified on the device):** SURFACE-4 (win),
  SURFACE-10 (mac), ADAPTERS-4 (mac), ADAPTERS-1 (mac), LIFECYCLE-1 (win), LIFECYCLE-2 (mac), STATE-14 (mac),
  SURFACE-6 (mac), SURFACE-7 (mac), ADAPTERS-9 (win), SURFACE-12 (mac).
- **Carry an open upstream PR instead of rewriting:** REFS-1 (`#588`), SURFACE-8 (`#549`), ACTION-5 (`#606`),
  LIFECYCLE-5 (`#582`, mac), STATE-6 (`#498`).
- **Open, blocked on a live probe (evidence first, no blind fix):** ADAPTERS-2 and BINARY-12 (Mac), ADAPTERS-3
  (Java ATK app on Linux), ADAPTERS-6 (Mutter closed-session error name), LIFECYCLE-12 (Mac), BINARY-8 (silent bus),
  REFS-6 (measurement).
- **Owned elsewhere:** MAC-SAVE-1 (Mac speed session).
- **Accepted as D / docs only:** STATE-18 (address-keyed FFI registry, misuse only: document), STATE-11 and ACTION-6
  (refactors with no reachable illegal transition left after #1/#10).

Pace so far: about 9 merged PRs in one long day with 3 writers in parallel (each PR 1 to 3 review rounds plus CI).
The fixable-here list (about 32 rows, roughly 22 PRs after grouping) plus 11 drafts and 5 carries is about 2 more
working days at that pace; the probe-blocked rows need the Mac or a Java app on Linux and will stay open until then.

## Upstream policy (George, 2026-11-07)

Nothing goes to vercel-labs yet: no PRs, issues, comments, or links that create timeline events. Every fix stays
upstream-ready: one finding per branch cut from `main`, no dependency on fork-only work (browser crate, perf
branches), upstream references as inline code, PR prose via the technical-writer skill. The Upstream column is
`ready`, `needs-issue-first` (JSON contract change or large refactor), `overlaps #N`, or `fork-only`.

## Posting rule (George, 2026-11-07, corrected)

Before any `gh pr create`, `gh pr edit`, `gh pr comment`, `gh issue ...` and before pushing any commit: write the body
or message to a file and run `grep -nE '@[A-Za-z0-9-]+|vercel-labs' <file>`. Zero hits outside backticks, or do not
post or push (a pushed commit that names an upstream issue adds a reference on its timeline). Post with
`--body-file`. Upstream authors in plain text, PR and issue numbers in inline code.
Checked 2026-11-07: no pushed commit message on any audit branch hits; `main` has only GitHub's automatic
`Co-authored-by:` email trailer on the PR #1 squash commit; PR #1's only hit is inside backticks.

## CI block (2026-11-07, from the control thread)

GitHub blocked Actions on the fork (`workflow_dispatch` returns HTTP 422 "Actions has been disabled for this
repository"; pushes create no runs). Do not work around it. Until George says it lifted: no merges, no dispatches or
re-runs; keep running local gates and pushing at checkpoints. PR #2 had merged just before (its dispatch run on head
`de04c3a9` completed 13/13 green).
**Lifted the same day:** Actions re-enabled with George's approval; normal merge rules apply. Dispatch CI only when a
head is ready for review, not on every push.

## Merge rule (George, 2026-11-07, updated)

Merge own fork PRs when all hold, and name the rule used in the PR body:
1. The head has the latest `main` merged in, local gates passed on it, and CI is 13/13 green on that head.
2. GitHub reports `CLEAN`.
3. If `main` moved during that CI run, the files changed on `main` since then do not overlap the PR's files. Always
   overlap: `Cargo.toml`, `Cargo.lock`, `.github/**`, `crates/core/src/lib.rs`, `AGENTS.md`, and skill files the PR
   touches. Any overlap re-cycles (merge `main`, gates, CI).
Plus: reviews done, not a draft; `gh pr merge <n> --squash --delete-branch`; watch `main`'s CI after each merge and
fix it if red. A PR that touches root `Cargo.toml`: first confirm Release shows `disabled_manually`.
CI and pushing (control room, approved by George): push each head as soon as it is ready so CI runs in parallel with
the local gates; no manual `workflow_dispatch` (`pull_request` CI triggers on the fork); a flaky timing test that turns
CI red gets its own small test PR (like #19), not a re-run; one PR merged at a time.
Host set since `a1873d35` (fork PR #8): add `-p agent-desktop-web`; `audit/tools/gates.sh` does it when `crates/web`
exists and also runs CI's core Win32-leakage check.

## Incidents

- 2026-11-08: merged PR #51 while GitHub still reported `UNKNOWN` (not yet recomputed after PR #53 merged seconds earlier), and without first waiting for `main` CI on `d921b52e`. Rule 3 held (no overlapping files). From now: re-read `mergeStateStatus` until `CLEAN` and wait for `main` CI between merges.
- 2026-11-08: merged PR #39 right after PR #40 without first waiting for `main` CI on `a3183289` (rule: watch `main` CI after each merge). Both PRs are test-only; `main` CI on both commits checked afterwards.
- 2026-11-07: the harness flagged the STATE-9 writer for a possible "security weakening" action. I stopped it and
  checked: nothing outside the worktree changed (`~/.claude/settings.json` untouched since Nov 3, `core.hooksPath`
  intact, lock dir still 0700), its last commit was not pushed, and its code matched the decided design. A
  read-only security review of that commit found no deliberate weakening but one real hole (Shared reads run the
  Linux stale-input sweep under the adopted lease); a fresh writer fixes it in round 4.

- PR #1's first body (2026-11-07) mentioned the upstream author with a bare @-handle and linked upstream PR 602 by
  URL. Fixed within a minute, but GitHub already notified the author and recorded a cross-reference on upstream
  #602's timeline; that event cannot be removed from the fork. All later PR bodies reference upstream items in
  inline code only (no autolink, no backlink).

## Found (outside scope)

- Unattended night run 2026-11-08: the Claude Code process restarted several times and stopped every background writer
  each time; work survived in the worktrees (uncommitted) and writers were resumed. Writers now commit and push early. Restarts traced to memory pressure (4-5 parallel Rust builds on 14 GB, swap full); limit is now two builds at a time.
- CI flake queue (each gets a small test PR): `wait_selector` gone-wait 250 ms (PR #29, also hit the Mac session's
  `feat/press-menu-shortcut`); npm bundled fixture race (PR #31); Windows `context::scope_tests::a_command_does_not_trace_into_a_session_reaped_while_it_waited_for_its_lease`
  failed once on `main` run 42450666424 (not yet fixed); `application_surface_tests::a_missing_document_window_does_not_satisfy_a_menu_disappearance_wait` (30 ms, under local load), `failed_inventory_poll_never_reports_app_termination` (under local load), `session::gc_tests::gc_removes_ended_sessions_but_not_pointer_or_live` and `dispatch::session::tests::session_gc_ended_reclaims_an_ended_session_that_recorded_trace_events` (same time-dependent GC liveness rule, see below).

- Root-caused in PR #40 (forked child holds the inherited lock until exec). Flaky `session::gc_tests::gc_removes_ended_sessions_but_not_pointer_or_live` (failed on Linux CI for PR #14's head
- Flaky `agent-desktop --test cli_process batch_session_ended_skip_*` (3 hits: PR #40 gate, w24 gate run 1, PR #42 gate): batch envelope had no `data.results` after ~5 s. Every hit was while two gate runs overlapped; 0 failures in 22 isolated runs (alone, under 16 busy loops, full binary x10), and holding the interaction lock does not trigger it. Mitigation: `gates.sh` now serializes itself (`flock` on `/tmp/claude-1000/gates.serial.lock`). Root cause still open (shared resource between concurrent `private-session.sh` runs suspected). 4th hit (PR #52 gate, 2026-11-08 ~04:40) happened with only writer builds running, so overlap is not the cause; `end_session` waits up to 5 s (`Deadline::standard`) on the session `refstore.lock` (`liveness::lock_started_session`), which fits the 5 s failures; 0 of 25 further loops reproduced. Next: a diagnostic test PR that prints the envelope on failure. **Root cause (2026-11-08 ~06:00, from PR #53's envelope):** batch-level `TIMEOUT` after 5.0 s, `kind: deadline`, `not_delivered` = the top-level batch's permission preflight (`main.rs`, `Deadline::standard()`) on a freshly started private session under load. Fix: PR #57.
  `075586f6` and once locally on plain `main`): `session/gc.rs:59-82` keeps an ended session that shows recent
  trace or snapshot activity, and a session started and ended within the same moment can count as recently active.
  A time-dependent liveness rule; belongs with STATE-6 (GC on manifest-less and stale state).

- Test hygiene: `crates/core/src/trace/trace_read/input_tests.rs:6` (`trace-input-*`) and the `agent-desktop-trace-mod-*`
  tests create directories in the system temp dir and never remove them. One day of gate runs here left 792 + ~900
  directories (about 4.6 GB) in `/tmp` and hit the per-user tmpfs quota (`Disk quota exceeded`, failing
  `embed_budget_skips_later_screenshots`). Removed today's leftovers; the tests need a drop guard.
- Two wall-clock wait tests are too tight for the macOS CI runner (`element_wait_explicit_session_snapshot_with_matching_session_context`
  at 50 ms, `dismissed_then_identical_notification_is_matched` at 600 ms); fixed in PR #19.

- Linux E2E baseline on this laptop (`main` `ec632e97`, x11-openbox): 93 pass, 55 fail, 1 error, all in leg setup
  (list in `audit/E2E-main-ec632e97-openbox-failures.txt`). Compare branches against it, not against zero.
  `ref_actions.double_click` flipped between runs (flaky).

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
