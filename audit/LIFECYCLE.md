# LIFECYCLE findings (app and window lifecycle)

Repo `/home/george/Projects/agent-desktop` at `b43ac4f6` (equals `upstream/main` at audit time). Read-only audit: no builds,
no tests run. Paths are relative to the repo root.

## Upstream register #9-#20 in this area
None of the open upstream findings #9-#20 touch launch, close-app, focus, window ops, process identity or renderer
activation. The register's SM10 entry calls `launch` and `close_app` "linear sequences". That holds for core. It misses the
poll loops inside each adapter, and those loops hold most of the findings below. `upstream/laf/state-machine-fixes`
(1170bcbc) touches none of the files in this area (checked with `git diff --stat b43ac4f6 1170bcbc`).

## Prior-art searches (shared, read-only)
Searches on `vercel-labs/agent-desktop`, issues and PRs, `--state all`, using these keyword sets: "launch stub exited",
"launch timeout pid", "launch calc", "launch window wait", "launch exited before window", "launch interaction lease",
"lease held launch", "close-app prompt", "close-app timeout", "quit prompt dialog_pending", "close-app already gone",
"renderer activation", "AXEnhancedUserInterface", "accessibility activation lease", "chromium settle",
"window move animation", "readback_mismatch", "restore window place", "maximize restore", "focus-window settle",
"Window focus did not settle", "process_state unresponsive", "APP_UNRESPONSIVE", "launch activate",
"launch AMBIGUOUS multiple windows", "not on the accessibility bus", "settle budget". Also:
`gh pr list --repo gshost1/agent-desktop --state all` (no PRs), `git branch -r` (origin has only
`docs/state-machine-ledger`, no `perf/` branches), and `git log upstream/main --oneline -S` for `observe_window`, `settled_window`,
`wait_for_exit`, `renderer_activation_attempted`, `SETTLE_BUDGET`, `PlaceStore`, `FOCUS_CONFIRMATIONS` and `find_owner`. No
merged fix was found for any finding. Raw output: `/tmp/claude-1000/-home-george--t3-scratch-2026-11-07-goal-find-every-implicit-state-1f4e3925/11164cc5-bf9f-4a7c-bbbc-ec7717576a36/scratchpad/lifecycle-priorart.txt`.

---

### LIFECYCLE-1: Windows launch has no "launcher exited / handed off" state (#552)
- Pieces: `crates/windows/src/system/launch.rs:36-41` (`launch_app_impl`). `:48-67` `observe_window` (the poll loop,
  `poll_interval` 50 to 250 ms). `:121-163` `observe_window_once` and `exact_window`, which match `window.pid == pid` only.
  `:240-244` closes `hProcess` right after `CreateProcessW`, which discards the one handle that could observe the exit.
  `:366-368` `should_poll_after_first_observation` (`timeout_ms > 0`). `launch_attach.rs:50-55` uses the same loop for
  attach.
- States today (implicit, spread across locals): `Spawned(pid, token)` → loop{`WindowSeen` | not seen → sleep} →
  `Deadline` → `Ok(window: None)`. No state exists for `Exited`, `HandedOff(owner)` or `Windowless`.
- Illegal transitions reachable:
  1. A stub exits and hands off to a packaged app: `Spawned` → (stub gone) → keeps polling for a window owned by the dead
     pid → `Deadline` → `ok: true` with the stale pid. Path: `launch_app_impl:39-40` → `observe_window:56-66` → `Ok(None)`
     → `launch_result` → core `launch.rs:44-56` returns ok. Input: `launch calc` on Windows 11. CONFIRMED (traced end to
     end; the reporter measured 30274 ms against a window ready at ~1.1 s).
  2. Attach to a running instance that has no window (a tray-resident app): `prepare` → `observe_window(row.pid, ...)` →
     full timeout → ok, window None. Linux returns after one read in this case (`wait_for_window` with `waits=false`) and
     macOS attaches without waiting (`launch.rs:31-37`). CONFIRMED (code trace).
  - Test sketch: none is possible on main without a seam, because `observe_window` calls `list_windows_live` directly.
    Extract it to `observe_window(read: FnMut, alive: FnMut, ...)`, as Linux `wait_for_window` does. Assert that
    `alive() == false` with `read() == None` returns `Err(exited_before_window)` within one poll. The extracted main logic
    has no `alive` input, so that test fails.
- Hidden cost: the whole `--timeout` (CLI default 30000 ms, `src/cli_args/system.rs:6-8`), measured by the reporter. At
  the 250 ms poll cap that is about 120 full `list_windows_live` desktop enumerations per launch (estimate). The
  machine-wide interaction lease is held the whole time (see LIFECYCLE-3).
- Impact: B. A stale pid and `ok` are reported after the full timeout. The action itself happened.
- Proposed explicit form: port the Linux `Step` machine (`crates/linux/src/system/launch/launch_observe.rs:33-67`,
  `launch_handoff.rs`) to Windows: `LaunchStep { Wait, Owner(Running), Ambiguous(pids), Failed(ExitStatus) }`, fed by a
  retained child handle (`WaitForSingleObject`/`GetExitCodeProcess`; `process_state::state_from_exit_code` already
  exists) and a before/after process baseline. Behavior fix S: keep `hProcess`, check exit on each tick, and fail with
  `exited_before_window` or search for a new matching window. The full handoff with the step enum is M. The attach fix
  (one read, no poll) is S. Files: `crates/windows/src/system/launch.rs`, `launch_attach.rs`.
- Overlap: #552. PR #483 (open, hosted-app attribution in `close.rs` and `window_ops.rs`) is needed for a handoff search to
  attribute `CalculatorApp.exe`. A perf change also touches LIFECYCLE-3 (lease scope).
- Prior art: #552 (open issue), #555 (open, hosted-app identity), PR #483 (open). No fix PR for the exit or handoff part.

### LIFECYCLE-2: macOS launch stops watching for exit once startup is "Finished"
- Pieces: `crates/macos/src/system/launch.rs:57-87` `settled_window` (locals `poll_interval`, `grace_ends_at:
  Option<Instant>`, `last_read_failure`, plus `options.activate` and `options.timeout_ms`). `:99-113`
  `startup_finished`, which is the only place that turns "process gone" into an error. `:357-376` `exact_window`, where a
  dead pid gives `WindowNotFound` and then `Ok(None)`. `window_inventory.rs:164-195`.
- States today: `Starting` → (`startup_finished` true) → `Grace(until)` → `GraceOver` → `Ok(None)`. With `activate=true`,
  `Grace` never ends and the loop runs to the deadline. `startup_finished` runs only while `grace_ends_at.is_none()`
  (`:76`), so exit detection is off once the state reaches `Grace`.
- Illegal transitions reachable:
  1. `launch --activate X` where X finishes launching and then quits (a helper or launcher app, or an app that crashes
     after startup). The loop polls the dead pid until the deadline and returns `ok` with `pid` of an exited process.
     CONFIRMED (code trace: `:76` guard, then `:70` `grace_over` is skipped because `activate`, so the loop exits only at
     `:79-81`).
  2. Without `--activate`, an exit inside the 1.5 s `STARTUP_GRACE` returns `Ok(None)` → `ok` with a dead pid (faster,
     same wrong state). CONFIRMED (code trace).
  - Test sketch: extract the loop into a pure step over `(window_read, startup_state, alive)`, the same seam as
    LIFECYCLE-1. Feed `Finished` then `alive=false`. Expect `Err(launch_target_gone)`. Main has no liveness input after
    `Finished`, so the test fails.
- Hidden cost: up to the whole timeout (30 s default) when `--activate` is set (estimate). The loop also re-reads the CG
  window list and AX state per poll (25 to 250 ms).
- Impact: B.
- Proposed explicit form: `enum LaunchWait { Starting, Grace { until }, AwaitingActivation }` with one
  `fn next(self, obs) -> Step`, where `obs` always includes `alive = matches_instance(pid, token)`. `Gone` from any state
  becomes `launch_target_gone`. S for the behavior fix (check `matches_instance` every tick), M if shared with
  LIFECYCLE-1 as a core pure `LaunchStep`.
- Overlap: LIFECYCLE-1 and LIFECYCLE-13 (same machine on three platforms).
- Prior art: none found. Searches: "launch exited before window", "launch window wait", "launch activate", and
  `-S settled_window` (last touched by #125, #206, #311, none of which fix this). PRs #553 and #451 are about the inventory,
  not this loop.

### LIFECYCLE-3: launch holds the machine-wide interaction lease through every passive wait
- Pieces: `crates/core/src/commands/launch.rs:43-50`. The `lease` local lives until `execute` returns, so it covers
  `adapter.launch_app` (spawn and the whole window poll) and `verify_cdp_endpoint` (`cdp_endpoint::probe` up to
  `MAX_PROBE_RESERVE` 5000 ms, plus `process_owns_tcp_listener`). The FFI path is the same:
  `crates/ffi/src/apps/launch.rs:67-79`. Compare the documented rule for activation, `crates/core/src/adapter/system.rs:41-43`:
  "Readiness polling belongs to core after the interaction lease is dropped".
- States today: `LeaseHeld` covers `Dispatch` → `ObserveWindow` → `ProbeCdp` → `Return`. Nothing models where
  `Dispatch` ends.
- Illegal transitions reachable: another agent-desktop process (another agent, or an MCP call) asks for the lease while a
  launch is waiting. It blocks until its own deadline and fails `TIMEOUT`, even though nothing is being mutated. With
  LIFECYCLE-1 that lasts 30 s, and each click, type or move from other agents in that window fails. CONFIRMED (code trace;
  the lease is cross-process: `session/interaction_lease.rs` flock on `/tmp/agent-desktop-<uid>/interaction.lock`, and the
  Windows `interaction_lease/`).
  - Test sketch (core, fails on main): copy `launch_cdp_test_support::CdpAdapter` with an `AtomicBool lease_held` set by
    the guard in `acquire_interaction_lease` and cleared on drop. Have `process_owns_tcp_listener` record `lease_held`.
    Run `launch --cdp` with `serve_cdp=true` and assert the recorded value is false. Main records true.
- Hidden cost: lease hold time per launch. Linux spawn: `WINDOW_GRACE` 2000 ms (or 5000 ms `NEW_WINDOW_WAIT`). macOS:
  startup plus `STARTUP_GRACE` 1500 ms. Windows: up to the whole timeout. `--cdp`: up to 5000 ms more (estimates from
  constants).
- Impact: B. Other commands get a wrong `TIMEOUT`. There is also C latency for concurrent agents.
- Proposed explicit form: split the adapter call into two phases: `launch_app(..., &lease) -> LaunchDispatched { pid,
  instance, baseline, child }` under the lease, then `observe_launch(dispatched, deadline) -> LaunchResult` without it
  (`activate`/focus re-takes the lease only for the focus write). Core drops the lease between the phases and runs the CDP
  probe without the lease. The typestate makes "observe under lease" unrepresentable. M (core, three adapters, FFI). Can
  be split: core-only S (drop the lease before `verify_cdp_endpoint`) first.
- Overlap: PR #436 (open) keeps `launch` on the machine-wide lock and touches `renderer_accessibility.rs`. LIFECYCLE-7b
  has the same "lease held for a wait" shape.
- Prior art: none found for launch lease scope. Searches: "launch interaction lease", "lease held launch". PR #436 is
  related but explicitly leaves launch on the global lease.

### LIFECYCLE-4: Linux launch of an app that never joins AT-SPI spends the whole timeout and then reports ok
- Pieces: `crates/linux/src/system/launch/launch_observe.rs:186-237` `find_owner`. Its loop ends only on `Owner`,
  `Ambiguous`, `Failed` or `Wait if expired`. `step` `:33-67`: a running child with no registry entry is always `Wait`.
  `mod.rs:283-298` `report_unlisted` and `:322`.
- States: `Wait`(child alive, not on bus) → … → `Deadline` → `Ok(report_unlisted)` with the suggestion "not on the
  accessibility bus yet".
- Illegal or unhandled transition: a live child that will never appear (a plain X11 app such as xterm, an app with
  `NO_AT_BRIDGE=1`, Chromium without its a11y flags) costs the whole `--timeout` (30000 ms default). The result is `ok`.
  CONFIRMED (code trace). Whether a given app joins the bus is environment-dependent.
  - Test sketch: `launch_observe_tests` with a fake registry that stays empty and a child that stays running, using a
    500 ms deadline. Assert that `find_owner` returns before `deadline - 400ms`, for example after a bounded "not on bus"
    grace. Main returns only at the deadline.
- Hidden cost: 30 s per such launch, with full `read_processes` registry walks at 50 to 250 ms (about 120 walks,
  estimate), all under the lease (LIFECYCLE-3).
- Impact: C (B-adjacent: the result is `ok` with a possibly-wrapper pid, though the suggestion is honest).
- Proposed explicit form: add `Step::NotOnBus { since }` with its own bounded grace (for example the same 2 s
  `WINDOW_GRACE`, longer with `--activate`). Settle it to `report_unlisted` early when the child is alive and nothing new
  matched. S.
- Overlap: LIFECYCLE-1 and LIFECYCLE-2 (same wait-policy family).
- Prior art: none found. Searches: "not on the accessibility bus" (found #460 and #446, which are unrelated inventory and
  WebKit issues), "launch window wait", `-S find_owner` (only #311).

### LIFECYCLE-5: close-app outcome is a bool; "prompt pending" exists only on Linux (#581)
- Pieces: core `crates/core/src/system/operation_report_dispatch.rs:42-62` (`from_close_dispatch(app,
  Result<bool,_>)`). macOS `crates/macos/src/system/app/close_outcome.rs` (`AlreadyGone|Exited`) and
  `app_ops.rs:130-158` `wait_for_exit`, which only polls `still_matches` every 25 ms and then returns a generic
  `TIMEOUT`. Windows `crates/windows/src/system/close.rs:236-264` `wait_for_exit` (same), `close_outcome.rs` and
  `close_dispatch.rs`. Linux `crates/linux/src/system/close/graceful.rs:113-200` (`Waited { Gone, Dialog(Standing),
  TimedOut }` and `refusal::dialog_pending`). The core contract comment `crates/linux/src/system/close/mod.rs:37-39`
  ("true only if a close request or signal was delivered") disagrees with core's reading of `true` as "exit observed".
  The Linux paths happen to return `true` only after `wait_gone`.
- States today: macOS and Windows have `Requested` → {`Exited` | `TIMEOUT`}. Linux has `Requested` → {`Gone` |
  `Dialog` → `dialog_pending` | `TimedOut`}, plus a pre-request `Dialog` refusal. Core can represent only `Exited`,
  `AlreadyGone` or an error.
- Illegal or unhandled transitions: on macOS and Windows a graceful close that raises a save or quit prompt ends in
  `TIMEOUT` with "may be busy or unresponsive" and `retry: unsafe`. A pre-existing modal also gets the close request
  (Linux refuses it, not delivered). The caller is steered to wait or `--force`. CONFIRMED (traced: `app_ops.rs:139-151`
  and `close.rs:247-258` have no surface read). The issue reproduced it 6 of 6 times on TextEdit. Windows Notepad
  "Save changes?" is the same path (SUSPECTED, no repro).
  - Test sketch: macOS has no seam (`wait_for_exit` calls `still_matches` directly). Core: add
    `close_app_tests::adapter_prompt_pending_maps_to_dialog_pending`, which needs the new enum. The Linux behavior is
    pinned in `graceful_tests.rs` and can be ported as the shared spec.
- Hidden cost: the whole remaining close budget (at most 5000 ms, LIFECYCLE-6) is spent before a prompt is reported.
- Impact: B.
- Proposed explicit form: a core `enum CloseOutcome { AlreadyGone, Exited, PromptPending { titles, raised_by_request:
  bool } }` replacing `Result<bool,_>` in `from_close_dispatch`, with `PromptPending` mapped to `ACTION_FAILED`
  `cause: dialog_pending` (pre-existing gives not_delivered; raised gives delivered_unverified, retry unsafe). Each
  adapter supplies a `prompt_standing(pid, instance, deadline)` probe that is checked every N ticks, as Linux does with
  `DIALOG_LOOK_EVERY`=4. S core, S per adapter. This also removes the Linux bool-meaning mismatch.
- Overlap: #581. PR #582 (open, macOS only) adds `crates/core/src/response/dialog_pending.rs` and probes at deadline
  expiry only. That gives the right error but still burns the whole budget, and leaves Windows unchanged.
- Prior art: #581 (open), PR #582 (open, partial: macOS, probe at deadline only). Linux `dialog_pending` is already on main.

### LIFECYCLE-6: close-app runs every phase on one 5 s budget; macOS gives a wrong error when the app exits mid-close
- Pieces: `crates/core/src/commands/close_app.rs:13-30`. A single `Deadline::standard()` (5000 ms,
  `wait/deadline.rs:8`) covers `resolve_app` → `acquire_interaction_lease(deadline)` → `revalidate_app_for_mutation` →
  `adapter.close_app` (request plus exit wait). There is no `--timeout` on `CloseAppArgs` (`src/cli_args/system.rs:66-75`).
  macOS: `app_ops.rs:52-55` calls `require_core`, which errors `APP_UNRESPONSIVE` "Target process instance is no longer
  running" (`process_identity.rs:129-145`) before `terminate_running_application` can return `AlreadyGone`.
- States: one deadline drives five phases. Time spent waiting for the lease is taken from the exit-confirmation phase.
- Illegal transitions reachable:
  1. The lease is contended (another agent acting, or LIFECYCLE-3). Close-app gets the lease late, sends the quit with
     about 0 ms left, and `wait_for_exit` returns `TIMEOUT delivered_unverified retry: unsafe` while the app quits a moment
     later. CONFIRMED (trace). Practical frequency depends on lease contention.
  2. macOS: the app exits between core's revalidate and the adapter (it quit on its own, or a second close-app ran).
     The result is `APP_UNRESPONSIVE` (disposition unknown) instead of `satisfied_without_delivery` "already gone".
     Linux and Windows return `AlreadyGone` here. CONFIRMED (trace), race-only.
  - Test sketch (core, fails on main): a stub adapter whose `acquire_interaction_lease` sleeps 4900 ms and whose
    `close_app` asserts `lease.deadline().remaining() >= EXIT_RESERVE`. On main remaining is about 100 ms.
- Hidden cost: none extra. The issue is budget allocation.
- Impact: B.
- Proposed explicit form: phase budgets like launch's `probe_reserve` (`launch.rs:61-63`). Pre-lease phases get
  `total - exit_reserve`, and the exit wait gets a reserved slice. Add `--timeout`. macOS: replace `require_core` with
  `parse_token` + `still_matches` → `AlreadyGone`. S each.
- Overlap: LIFECYCLE-5 (same files). PR #436 moves close-app to an app-scoped lease, which shrinks contention but does
  not split the budget.
- Prior art: none found. Searches: "close-app timeout", "close-app already gone". #487 and PR #596 are CLI targeting only.

### LIFECYCLE-7: renderer accessibility activation state is split across three owners with different lifetimes
- Pieces: core `crates/core/src/tree/renderer_accessibility.rs:20-47` (per call `activated: bool`, `retry_delay` 25 to
  400 ms, a full `observe_tree` per retry). `:49-63` `activate` extends the deadline by the activation time
  (`extended_by`, bounded only by an inherited batch deadline, `wait/deadline.rs:117-130`). `:65-79` `switch_on`
  acquires a fresh lease with `Deadline::standard()` (5000 ms), not the command deadline. Windows
  `crates/windows/src/adapter.rs:22-49` keeps `renderer_activation_attempted: Mutex<HashSet<ProcessIdentity>>` for the
  adapter's lifetime. `crates/windows/src/system/adapter.rs:76-88` inserts and then sleeps 50 ms under the lease.
  `crates/windows/src/tree/observe.rs:100-114` gates on it. macOS `crates/macos/src/system/renderer_activation.rs:7-30`
  ("flag reads true" → done). `:36-76` writes, reads back, then sleeps `ENHANCED_ACTIVATION_DELAY` 2000 ms while holding
  the lease. `crates/macos/src/tree/renderer_probe.rs:30-46` treats `enabled == Some(true)` as "not required". Linux has
  no activation path. The user supplies the env flags, see `crates/linux/src/system/app/app_ops.rs:10`.
- States (actual, per process): `NotRequired | Required | Requested(debounce until t+2s) | Active | Refused`. The code
  stores `Requested` only as a thread sleeping in one process. Across processes, `Requested` and `Active` both read as
  "flag true".
- Illegal or unhandled transitions:
  a. macOS `Requested` seen as `Active`. Process A writes `AXEnhancedUserInterface` and is killed during its 2 s sleep
     (MCP `notifications/cancelled` kills the child; Ctrl-C). Process B within 2 s reads true, so `activation_required` is
     false, and it acts on a readable tree whose AX actions are still inactive. The solution doc says this yields
     `delivered_unverified` clicks with no effect. SUSPECTED (needs a probe with a cancelled snapshot followed by a click).
  b. Windows: a read-only snapshot of a cold Chromium window needs the machine-wide lease for a no-op 50 ms sleep. If
     another process holds the lease (LIFECYCLE-3), `switch_on` waits up to 5 s and maps to `TIMEOUT`. CONFIRMED (trace:
     `observe.rs:110` → core `:33-35` → `switch_on:73` → `activate:56-60`).
     Test sketch (core, fails on main): a stub adapter whose `observe_tree` returns
     `renderer_accessibility_activation_required` once and whose `acquire_interaction_lease` returns `Timeout`. Call
     `observe_tree(..., lease: None)` and assert Ok after the retry. Main returns `TIMEOUT`. (The fix needs an adapter
     opt-out such as `fn renderer_activation_mutates() -> bool`.)
  c. macOS holds the lease for the 2 s debounce, which contradicts the trait doc (`adapter/system.rs:41-43`). All other
     agent-desktop interactions machine-wide block for 2 s. CONFIRMED (code).
  d. Deadline drift: a snapshot with `--timeout-ms 3000` can run for 3000 plus up to 5000 (lease wait) plus 2000
     (debounce). The time `extended_by` adds is uncapped for non-batch commands. CONFIRMED (trace). This is the intended
     "own budget" from #254, but unbounded.
  e. The Windows `HashSet` never shrinks in a long-lived FFI or MCP adapter. It duplicates core's `activated` flag at
     another scope. D.
  f. #465 (Arc): `Refused` is terminal for the whole snapshot (`ACTION_NOT_SUPPORTED`). There is no "native part only,
     web area unavailable" outcome. This is product scope, recorded for completeness.
- Hidden cost: macOS 2000 ms under the lease on the first activation of each Chromium process (constant). Windows 50 ms
  under the lease plus lease acquisition. Every retry re-walks the full tree (backoff 25, 50, 100, 200, 400, 400… ms). A
  5 s budget gives about 13 walks (estimate).
- Impact: B (a, b). C (c, d). D (e).
- Proposed explicit form: activation becomes a two-phase trait call:
  `activate_renderer_accessibility(process, &lease) -> ActivationRequested { settle: Duration, mutated: bool }`, plus
  `fn renderer_activation_needs_lease(&self) -> bool` (Windows false). Core owns `enum ActivationPhase { NotTried,
  Settling { until }, Settled, Refused }` per call, and waits out `settle` after dropping the lease, with the wait charged
  to a capped activation budget. This removes the Windows `HashSet`, because core passes "already settled this call" in
  the `ObservationRequest`. For (a), persist `settle_until` keyed by `ProcessIdentity` under the state root, or have macOS
  always wait the residual debounce when it cannot prove the write is older than 2 s. M. (b) and (c) can be split as S
  behavior fixes.
- Overlap: PR #436 touches `renderer_accessibility.rs`. #254 (closed) is the budget history. #465. Docs:
  `docs/solutions/runtime-errors/chromium-readable-tree-with-inactive-ax-actions.md`.
- Prior art: #465 (open), #238 (closed), PRs #241 and #221 (closed), #254 (closed, "give accessibility activation its own
  budget"), #117 (merged). None addresses lease scope or cross-process debounce. Searches: "renderer activation",
  "AXEnhancedUserInterface", "accessibility activation lease", "chromium settle".

### LIFECYCLE-8: Linux window-op settle: a placement that lands after ~130 ms can never be verified
- Pieces: `crates/linux/src/system/window/window_op/window_op_verdict.rs:16-18` (`POLL` 25 ms, `SETTLE_BUDGET`
  250 ms). `mod.rs:39` (`CONFIRM_PAUSE` 120 ms). `mod.rs:172-207` `settle`: `limit = deadline.capped(SETTLE_BUDGET)`.
  On the first `Satisfied`, `if limit.remaining() < CONFIRM_PAUSE → Mismatch`. `window_op_hold.rs:3-16`
  `confirm_rectangle`. `Verdict { Satisfied, Pending, Mismatch }` has no `Provisional`, so the CONCEPTS.md "Window
  Rectangle" provisional state lives in the control flow.
- States: `Pending` → `Satisfied`(first match, really Provisional) → confirm pause → `Satisfied`(held) or `Mismatch`. The
  limit is fixed at 250 ms whatever the command budget (5000 ms).
- Illegal transition reachable: the WM applies a move or resize at t ≥ 131 ms (a loaded box, Xwayland, a compositor
  animation). The first matching read leaves less than 120 ms in the limit, so the result is `ACTION_FAILED
  readback_mismatch delivered_unverified`, with `observed` equal to `requested`. The request did take effect.
  CONFIRMED (deterministic trace). Real-world frequency SUSPECTED, WM-dependent.
  - Test sketch (fails on main): `window_op_settle_tests` with `World::showing(old)` repeated about 6 times then
    `then_showing(target)` (first match at about 150 ms), deadline 2000 ms. Assert that `run(..., Move)` is Ok. Main returns
    Err. Existing tests `a_confirmation_that_agrees_but_returns_after_the_limit_is_not_a_held_place` pin "past the limit
    is unverified". The finding is that the limit is a fixed 250 ms instead of "first match + confirm pause, within the
    command deadline".
- Hidden cost: none. The cost is a false failure and a likely redundant retry by the caller.
- Impact: B.
- Proposed explicit form: `Verdict::Provisional { since: Instant }`. The transition `Provisional` → `Held` needs
  `CONFIRM_PAUSE` of stability, and the settle limit extends to `max(SETTLE_BUDGET, since + CONFIRM_PAUSE)` capped by the
  lease deadline. S, `window_op_verdict.rs` and `mod.rs`.
- Overlap: LIFECYCLE-11 (the same rule, implemented differently per adapter).
- Prior art: none found. Searches: "window move animation" (found PR #393, macOS minimize animation, related), "settle
  budget", "readback_mismatch".

### LIFECYCLE-9: Linux remembered pre-maximize place is never invalidated, so restore moves the window to a stale place
- Pieces: `crates/linux/src/system/window/window_op/mod.rs:208-210` (`remember_place` on maximize). `:254-283`
  `restore`: a window that is not maximized returns `give_back_place` (`:256-258`), and so does the minimized →
  lifted path. `window_op_hold.rs:51-63` `give_back_place`, `:111-119` `remember_place` (only when not maximized and not
  hidden). `window_place.rs` on-disk store keyed by client and instance, dropped only when the process dies (`sweep`).
- States: `NoPlace` → (agent maximize) → `Remembered(P1)` → (agent restore) → `NoPlace`. A user or WM un-maximize does
  not move the state off `Remembered(P1)`.
- Illegal transition reachable: the agent runs `maximize`, the user un-maximizes (double-click the title bar) and drags
  the window to P2, and later the agent runs `restore` (or `minimize` then `restore`). The window is moved to P1 and the
  operation reports success. CONFIRMED (code trace). The on-disk state disagrees with WM state, and a move is delivered
  that nobody asked for.
  - Test sketch (fails on main): in `window_op_tests`, the fake `World` with `remember`/`remembered` (`:189-196`). Run
    Maximize, then switch the fake state to unmaximized at P2, then run Restore. Assert `world.writes()` shows no
    `MoveResize` to P1. Main writes it.
- Hidden cost: none.
- Impact: A (a wrong action delivered; low severity).
- Proposed explicit form: store `{ place_before, rect_after_maximize }` and give the place back only from
  `Maximized`, or when the current rect still equals what the agent last left. `enum PlaceMemory { None,
  BeforeMaximize { place, maximized_rect } }`, cleared whenever an op starts and sees the window un-maximized by someone
  else. S.
- Overlap: none.
- Prior art: none found. Searches: "restore window place", "maximize restore" (#487 and PR #596 are CLI targeting only).

### LIFECYCLE-10: focus-window confirms twice; the second layer has no reserved budget
- Pieces: core `crates/core/src/system/window_focus.rs:5-7` (`FOCUS_SETTLE_TIMEOUT_MS` 750, poll 50,
  `FOCUS_CONFIRMATIONS` 2). `:9-21` calls `adapter.focus_window` and then the core wait. Trait contract
  `adapter/system.rs:68-70`: "implementations must return only after that window is confirmed focused". Linux
  `focus/focus_window.rs:22-27` (`FRAME_REPORT_GRACE` 6000 ms, bounded by the lease deadline). Command
  `commands/focus_window.rs:13` uses one `Deadline::standard()` for list, select, lease and focus.
- States: `AdapterConfirmed` → core `Confirming(n)`. The adapter's confirmation is ignored.
- Illegal transition reachable: a Chromium or Electron frame reports active late (Linux, 1.8 s or more), so the adapter
  confirms with less than ~50 ms of the 5 s left. Core gets at most one read and then returns `ACTION_FAILED "Window focus
  did not settle"` with disposition unknown, after a confirmed focus. CONFIRMED (trace). The window is narrow,
  load-dependent.
  - Test sketch (core, fails on main): a stub adapter whose `focus_window` sleeps until `lease.deadline().remaining() <
    30ms` and returns Ok, and whose `focused_window` returns the target. Assert `execute` is Ok. Main returns
    ActionFailed.
- Hidden cost: at least 50 ms of sleep plus 2 `focused_window` reads per `focus-window`, after the adapter already
  confirmed (estimate; constants measured from code).
- Impact: B (narrow) and C.
- Proposed explicit form: trust the adapter's confirmation (the contract), and have core do one corroborating read with
  a reserved slice taken before calling the adapter. Map a late failure to `delivered_unverified`. S.
- Overlap: none.
- Prior art: none found. Searches: "focus-window settle", "Window focus did not settle".

### LIFECYCLE-11: the "provisional placement" rule is implemented three different ways
- Pieces: Linux (LIFECYCLE-8). macOS `crates/macos/src/system/window/window_postcondition.rs:5-54` returns on the first
  match, with no confirm pause, polling every 10 ms. Windows `crates/windows/src/system/window/window_op_wait.rs:7-22`
  always sleeps 80 ms before the first read and returns on the first match, with an 8 px tolerance
  (`window_op.rs:14`).
- States: no shared type. Each adapter decides when a placement counts as held.
- Unhandled: macOS and Windows can report a verified hold on an animation frame that later snaps back. SUSPECTED (no
  repro). CONCEPTS.md states the rule for Linux only.
- Hidden cost: Windows pays a fixed 80 ms on every move, resize, minimize, maximize and restore, even when the placement
  already holds (constant). macOS `wait_for_minimized` stopping on the first read error is a separate bug fixed in PR
  #393 (open).
- Impact: C (Windows fixed sleep) and D (divergence).
- Proposed explicit form: a core `PlacementVerdict { Pending, Provisional{since}, Held, Mismatch }` with one pure
  `judge(prev, observed, now, policy)`. Adapters supply reads and per-platform tolerance and pause. M (touches three
  adapters). The Windows "read first, then sleep" change is S.
- Overlap: LIFECYCLE-8. PR #393.
- Prior art: PR #393 (open, macOS AXMinimized during animation). None for cross-adapter unification. Searches: "window
  move animation", "settle budget".

### LIFECYCLE-12: macOS launch fails AMBIGUOUS_TARGET after a successful launch when the app restores several windows
- Pieces: `crates/macos/src/system/launch.rs:63-67` (any `exact_window` error other than not-ready propagates).
  `window_inventory.rs:179-202` (several windows and no single focused one → `ambiguous_target`). The caller wraps it in
  `after_launch` → `delivered_unverified`.
- Transition: `Launched` → `WindowRead(Ambiguous)` → `Err(AMBIGUOUS_TARGET)`, while the app is up. Linux picks the first
  new window (`choose_window`). Windows takes the first window owned by the pid. SUSPECTED (needs an app restoring two
  windows with no AX-focused one while it launches in the background, `activate=false`).
- Impact: B.
- Proposed explicit form: inside the launch state machine (LIFECYCLE-2), `Ambiguous` → report `window: None` plus
  `candidate_window_ids` in the suggestion, not an error. S.
- Prior art: none found. Searches: "launch AMBIGUOUS multiple windows" (#494 and PR #523 are unrelated ref actions).

### LIFECYCLE-13: LaunchOptions encodes the wait policy in `timeout_ms` + `activate`, read differently per adapter (D)
- Pieces: `crates/core/src/system/launch_options.rs`. Core `launch.rs:21-26`: `timeout_ms == 0` means the 5 s standard
  deadline. The adapters read 0 as "observe once": Windows `launch.rs:366-368`, macOS `launch.rs:69`, Linux
  `mod.rs:29-35`. Windows ignores `activate` entirely (it always polls to the deadline). macOS: activate means "wait to
  the deadline". Linux: activate means "wait to the deadline and focus", otherwise 2 s or 5 s grace. The FFI doc
  (`ffi/src/apps/launch.rs:24-28`) says zero means "no wait".
- No reachable illegal transition beyond LIFECYCLE-1, 2 and 4. The policy is a hidden product of two fields and three
  adapters.
- Proposed explicit form: `enum WindowWait { None, Grace(Duration), UntilPresented }` resolved once in core from the CLI
  or FFI inputs and passed in `LaunchOptions`. S core, S per adapter.

### LIFECYCLE-14: ProcessState is stateless, but `Unresponsive` is derived from the caller's remaining budget (D, SUSPECTED B)
- Pieces: `crates/core/src/system/process_state.rs`. macOS `process/process_state.rs:16-30` (two consecutive
  `kAXErrorCannotComplete` mean Unresponsive), `:55-60` with `set_messaging_timeout(app, deadline)` using the caller's
  remaining deadline. `ref_action_wait_support.rs:6-41` (`enrich_with_process_state` has no probe cap, while
  `with_exited_process_state` caps at 1 s, `:99-118`). Windows `process_state.rs:24-38` (hang probe, also two passes).
- Unhandled: with a few hundred ms left, a briefly busy app can time out both AX probes and classify as `Unresponsive`
  (`retryable: false`). The `process_identity_matches` → `list_apps` guard usually fails on the same short budget and
  prevents it. SUSPECTED, low.
- Proposed explicit form: a fixed probe budget (for example 2×250 ms, detached from the caller deadline) for the
  responsiveness probe. S.

### LIFECYCLE-15: process identity and generation tokens, CDP endpoint ownership (D, checked)
- `ProcessIdentity { pid, instance }` (`core/src/system/process_identity.rs`) and the platform tokens (macOS
  `process_identity.rs:107-146`, Windows `process_identity.rs:93-231`, Linux `process/identity.rs`) are immutable values,
  and every mutation re-checks `still_matches` or `matches_instance` right before it writes. Close paths verify the
  handle's creation time before `TerminateProcess` (`windows/.../close.rs:325-354`). No reachable stale-generation write
  was found. One exception: Windows `handle_matches_instance` returns false when `GetProcessTimes` fails, which becomes
  `AlreadyGone`, so the result is "satisfied" for a live process (`close.rs:353-355`). That is unlikely, because the
  handle has QUERY_LIMITED.
- CDP: `pick_free_port` binds and releases (`cdp_endpoint.rs:29-34`), so another process can take the port before the
  app does. `verify_cdp_endpoint` then fails `cdp_listener_unattributed` because ownership is checked
  (`launch.rs:186-216`). The race is handled correctly.
- Linux AT-SPI enablement: none exists, and the adapter never flips `org.a11y.Status`. Nothing to model.

## Checked and found sound
- Close-app never reports `delivered_verified` while the target still runs. All three adapters derive `Exited` from an
  identity-checked gone observation (macOS `still_matches`, Windows `process_observed_gone`, Linux `liveness == Gone`),
  and Linux `force` returns Ok only once the process is observed gone or replaced.
- macOS `launch_completion.rs` outstanding-launch counter (static, 16): released by the bridge on every path, including
  allocation failure (`launch_bridge.m`).
- Linux launch handoff (`launch_observe.rs` `Step`, `launch_handoff.rs`) is the reference shape for LIFECYCLE-1 and 2.
