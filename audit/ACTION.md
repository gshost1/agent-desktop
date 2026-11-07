# ACTION area: implicit state machines between a resolved ref and a delivered result

Repo `/home/george/Projects/agent-desktop`, main = upstream/main = `b43ac4f6`. Paths are relative to
`crates/core/src/` unless they start with `src/` (binary crate). Read-only audit: nothing was built or run.
Costs are estimates from constants and from test-adapter read counts, not from live timing.

Upstream open findings in this area, checked against main:

| # | Verdict on main | Where it lands here |
|---|---|---|
| #17 | Still true, CONFIRMED. The post screenshot runs after the post-action wait against the spent action deadline | ACTION-3 |
| #18 | Still true, CONFIRMED. A lease failure, or a deadline that runs out right after the lease, drops a written pre screenshot | ACTION-3 |
| #19 | Still true, CONFIRMED, and wider than the register says: the baseline is also taken after the trigger | ACTION-4 |
| #10 | Still true, CONFIRMED. It also reaches a satisfied `--wait-for` | ACTION-8 |
| #6 (fixed on the unmerged branch) | Overlap only. The branch marks `stable_preflight` errors `not_delivered` (ref_action/mod.rs:94). ACTION-1 covers the other pre-dispatch refusals that branch leaves `Unknown` | ACTION-1 |
| issue #605 | Still true, CONFIRMED. Open PR #606 fixes `--action` and `--window-id` only | ACTION-5 |

Prior-art searches ran per finding, read-only: `gh issue list` and `gh pr list --repo vercel-labs/agent-desktop
--state all --search ...`, `gh pr list --repo gshost1/agent-desktop` (no PRs at all), `git branch -r` (only
`origin/docs/state-machine-ledger`; no `perf/` branches), and `git log upstream/main -S/--grep`. upstream/main
equals main, so nothing below is already fixed upstream. Nothing is DROPPED.

---

### ACTION-1: Pre-dispatch refusals that keep `delivery: unknown` (lease, post-lease deadline, headed focus, strict trace)

- **Pieces**
  - `ref_action/ref_action_wait.rs:97-106`: cursor hold, then pre-lease travel, then `acquire_interaction_lease(deadline)` with a `?` that only attaches metrics. After that, `deadline.is_expired()` returns `deadline.timeout_error()`.
  - `session/process_lease_guard.rs:46-51` (`interaction_process_lock_timeout`) and `storage/file_lock.rs:101-121,134` (`lock_timeout`): both build `deadline.timeout_error()` (`wait/deadline.rs:168-174`), which is `AdapterError::timeout` with `DeliverySemantics::Unknown` (`response/adapter_error.rs:196-199`).
  - `ref_action/ref_action_single.rs:29-52`: before dispatch, `focus_app(process, lease)?` and `focus_entry_window(...)` pass the adapter's or core's error through unchanged.
  - `actions/headed_focus.rs:14-20`: "Headed input requires target process-instance identity". The details say `physical_delivery_started: false`, but the error has no disposition, so it is `Unknown`.
  - `ref_action/mod.rs:138-141` (`action.dispatch.start` trace `?`) and `ref_action/ref_action_actionability.rs:37-40,62-64` (`actionability.check.*` trace `?`): a strict-trace failure is `AppError::Internal`, which `into_adapter_error` turns into `Unknown` (`ref_action/mod.rs:391-396`).
  - `commands/helpers.rs:328-377`: `load_ref_entry` trace `?` (strict trace).
  - `src/dispatch/disposition.rs:16-23`: `settle_unknown_delivery` keeps `Unknown` for mutating commands. `response/output.rs:58-64` gives no `recovery` unless retry is `safe`.
- **States and transitions today**
  ```
  Polling --ok--> PreArtifact --> CursorHeld --lease Err------------------> Failed{Unknown}   (nothing sent)
                                  CursorHeld --lease Ok, deadline spent---> Failed{Unknown}   (nothing sent)
                                  CursorHeld --lease Ok--> Focusing --focus Err--> Failed{adapter disposition, usually Unknown}
                                                           Focusing --> Dispatch: trace start Err --> Failed{Unknown}
                                                           Dispatch: execute_verified_action --> Delivered* | adapter Err
  ```
  The invariant in the upstream register (SM4: "any refusal raised before `execute_verified_action` must be `NotDelivered`") has no enforcement point. Each pre-dispatch step has to remember to mark its own errors.
- **Illegal transitions reachable**
  1. Lease contention: CONFIRMED (traced end to end). Two agent-desktop processes run; one holds `/tmp/agent-desktop-<uid>/interaction.lock` through a long action (`type --interval-ms`, a drag, a 5 s `--timeout` click). The other runs `click @s:e1 --timeout 3000`. The path is `execute_with_deadline` (ref_action_wait.rs:99-102), then `ProcessLeaseGuard::acquire` or `FileLock::lock_file`, then `lock_timeout`, then `Unknown`. The CLI envelope says `"retry":"unknown"` and carries no recovery hint, although nothing was sent. `resolves_again` is never consulted, so the auto-wait does not go back to waiting either. The `details.refusal` from an earlier refusal is also dropped on this path: `with_refusal` is applied at :86 and :105, not at :102.
     - Test sketch (fails on main): copy the adapter in `ref_action/ref_action_wait_lease_tests.rs`, make `acquire_interaction_lease` return `deadline.timeout_error().with_details({"kind":"interaction_process_lock_timeout"})`, and run `run_with_entry(..., 3_000)`. Assert `err.disposition == DeliverySemantics::NotDelivered` (today `Unknown`) and `execute_action` calls == 0.
  2. Deadline runs out while the lease is taken: CONFIRMED. ref_action_wait.rs:103-106 returns `deadline.timeout_error()`, which is `Unknown`.
     - Test sketch: `VirtualClock::freeze()`, and an adapter whose `acquire_interaction_lease` calls `clock.advance(deadline.remaining())` and then returns `InteractionLease::guarded`. Assert `NotDelivered`.
  3. Headed focus failure: CONFIRMED in core. `click --headed @ref`: `execute_single_shot` runs `focus_entry_window` → `resolve_window_strict` / `focus_window` → error (for example a `TIMEOUT` from the adapter) → returned as is, `Unknown`. The click was never sent. The window may have been raised, which is the same class of side effect as `preparatory_scroll`, and that one is marked `not_delivered` (ref_action_wait_support.rs:133-142).
     - Test sketch: use `exact_window_focus!` in `adapter/test_support.rs` and make `focus_window` return `AdapterError::timeout("x")`. Run a headed `Action::Click`. Assert `NotDelivered`, `details.preparatory_focus`, and `execute_action` never called.
  4. Strict trace before dispatch: CONFIRMED by code, SUSPECTED for reach in practice. It needs `--trace-strict` plus a full trace budget or a write error.
- **Hidden cost**: none. This is a correctness problem only.
- **Impact class**: B. It reports a retry-safe refusal as `retry: unknown` with no recovery. It cannot cause a double delivery.
- **Proposed explicit form**: one phase boundary instead of per-site marking. Wrap everything before `execute_verified_action` in a `PreDispatch<T>` result. Name the step `ref_action::pre_dispatch(|| ...)`, matching `pointer_action::before_first_event`. It converts any `Unknown` to `NotDelivered`, keeps stronger evidence, and records the side-effect step (`preparatory_focus`, `preparatory_scroll`). Use it for lease acquisition, the post-lease deadline check, focus, resolve, preflight and trace start. `dispatch_resolved_landing` then has exactly two zones, and `Unknown` cannot appear before dispatch. Also apply `with_refusal` on the lease error. Files: `ref_action/ref_action_wait.rs`, `ref_action/ref_action_single.rs`, `ref_action/mod.rs`, and a new `ref_action/pre_dispatch.rs`. Size S (about 80 lines plus tests). The behavior fix is the whole change; there is no separate refactor.
- **Overlap**: upstream #6 / issue #598 (same class; the branch fixes only `stable_preflight`). The interaction lease is SM4 in upstream's "Sessions and leases" section. A perf change to the lease or the auto-wait loop would touch `ref_action_wait.rs` too.
- **Prior art**: issue #598 (partial, preflight only). Nothing for lease, focus or trace. Searches: `"lease timeout retry unknown"`, `"interaction lease disposition"`, `"not_delivered lease"`, `"retry unknown"`, `"delivery unknown"`, `"focus failure disposition"`, `"headed focus not_delivered"`, `"trace-strict disposition"` (issues and PRs); `git log upstream/main -Sinteraction_process_lock_timeout`.

### ACTION-2: `type --interval-ms --wait-for` re-reads the refmap after every keystroke is delivered and can report `not_delivered`

- **Pieces**
  - `commands/type_text_interval.rs:49-79`: the burst. Partial failures are wrapped by `after_partial_delivery` (:267-285) as `delivered_unverified`.
  - :90-94: after the last keystroke, `load_ref_entry(&args.ref_id, context)?` runs with no post-delivery wrapper, then `apply_post_action_wait`.
  - `commands/helpers.rs:325-357`: `load_ref_entry` returns `AppError::stale_ref` (`not_delivered`, adapter_error.rs:92-102) or a `SNAPSHOT_NOT_FOUND` from `RefStore::load_snapshot` (`not_delivered`, adapter_error.rs:238-251).
  - `refs/refs_store.rs:16,58-63` and `refs/refs_store_prune.rs:72-92`: every new snapshot in the namespace prunes beyond `MAX_SAVED_SNAPSHOTS = 128`.
- **States and transitions today**
  ```
  Landing(unit0, full ref action) -> Burst(unit i, delivered_unverified on error) -> AllTyped
  AllTyped --no --wait-for--> Ok
  AllTyped --wait-for--> ReloadEntry --Err(not_delivered)--> Failed{not_delivered, retry: safe}   <- illegal
                         ReloadEntry --Ok--> PostActionWait (finish_post_action_wait copies the action's delivery)
  ```
- **Illegal transition reachable**: CONFIRMED (traced end to end) for the code path. Reach in practice is SUSPECTED: the snapshot has to disappear during the burst. A 600-character text at `--interval-ms 100` runs for about 60 s. In the same namespace (global, or a shared `--session`), 128 snapshot saves by other agents in that window prune the ref's snapshot. Saves come from snapshot, find, `wait --text` and `--wait-for` matches. The command then returns `SNAPSHOT_NOT_FOUND` with `delivery: not_delivered, retry: safe`, plus a recovery `refresh_snapshot_then_retry_original`. An agent that follows it types the whole text a second time. This is a wrong action delivered twice.
  - Test sketch (fails on main): use the test adapter from `commands/type_text_interval_tests.rs`. Its `execute_action` deletes the snapshot directory through `RefStore::for_session(None)` when it sees the last unit. Run `type --interval-ms 1` on "ab" with `context.with_wait_selector(Some(..))`. Assert `err.disposition != NotDelivered` (today `NotDelivered`).
- **Hidden cost**: one extra refmap load plus three trace events after the burst. `burst_around` (:183) already reloaded the same entry, and `execute_landing` loaded it before that: three loads of one entry per command.
- **Impact class**: A.
- **Proposed explicit form**: keep the `RefEntry` that `execute_landing` loaded. Return `(Value, NativeHandle, RefEntry)` from `execute_landing` and give the entry to `Burst` and to `apply_post_action_wait`. The reload goes away, and with it the illegal transition. Belt and braces: route any error after the first delivered unit through `after_partial_delivery(err, units.len(), units.len())`. File: `commands/type_text_interval.rs` only. Size S (about 20 lines plus 1 test).
- **Overlap**: none in the upstream register. Merged PR #348 and closed PR #362 (`laf/type-interval-wait-and-disposition`) moved the wait after the last keystroke, which created this reload.
- **Prior art**: PR #348 (introduced), PR #362 (closed; the same area, not this bug). Searches: `"interval-ms wait-for"`, `"type interval disposition"`, `"typed twice"`, `"snapshot not found after action"`; `git log upstream/main -Safter_partial_delivery`.

### ACTION-3: Pre/post action artifact pairing lives in loose locals (#17, #18, one screenshot pair per refusal)

- **Pieces**
  - `ref_action/ref_action_wait.rs:91-96`: `pre` captured on every iteration of the outer loop.
  - :99-106: early returns that never call `finish_artifacts`.
  - :123: `finish_artifacts` on refusal.
  - :25-29: the plain path.
  - `commands/helpers.rs:131-161`: success path. The lease is dropped (:147), `apply_post_action_wait` runs (:148, default 30 000 ms), and only then `finish_artifacts(..., deadline)` runs (:150-161).
  - `ref_action/mod.rs:199-259`: capture budgets. `TRACE_CAPTURE_BUDGET_MS = 1000`, `PRE_CAPTURE_DISPATCH_RESERVE_MS = 100`. `trace_capture_deadline = parent.capped(1 s)`.
  - `trace/trace_artifacts.rs:64-207`: `ArtifactOutcome::{Captured(String), Skipped(String)}`. Skip reasons are strings.
- **States and transitions today**
  ```
  per outer iteration: NoPre -> PreCaptured -> (lease Err | deadline spent) -> [orphan file, no event]          (#18)
                                            -> refusal -> PostCaptured -> Emitted -> next iteration NoPre
                                            -> Ok -> (CLI) wait... -> PostCaptured(after wait, action deadline) -> Emitted   (#17)
  ```
- **Illegal transitions reachable**
  1. #17: CONFIRMED. `click @ref --wait-for 'button:Done'` in a full-artifacts session. When the wait outlasts the action's remaining budget (5 s default), `trace_capture_deadline(deadline)` is already expired, so the post is `Skipped("deadline")`. When it does not, the "post" image shows the UI after the wait, not after the action.
     - Test: a full-artifacts session context, an adapter whose wait poll advances `VirtualClock` 6 s. Assert the `action.artifacts` event has `screenshot_post`. Today it has `skipped_post: "deadline"`.
  2. #18: CONFIRMED. The lease error, or the deadline check after the lease (ref_action_wait.rs:99-106), returns after `capture_pre_artifact` wrote `screens/<file>` and charged `.artifact-usage.json`. No `action.artifacts` event references the file.
     - Test: a lease that fails, artifacts enabled. Assert one `action.artifacts` event with `screenshot_pre`. Today there is none, and the screens directory has one file.
  3. Each auto-wait refusal re-captures pre and post. CONFIRMED by code.
- **Hidden cost (estimate)**: with full artifacts, each refusal spends up to 0.9 s (pre) plus 1 s (post) of the action's own deadline on screenshots and emits an extra `action.artifacts` pair. Two refusals can use most of a 5 s budget. With artifacts off the cost is zero (an early return at trace_artifacts.rs:71).
- **Impact class**: B for #17, which gives a misleading trace (the post frame is not the action's post state). C for the per-refusal capture. #18 is D (an orphaned file plus artifact budget).
- **Proposed explicit form**: an `ActionArtifacts` guard created after the poll: `ActionArtifacts::begin(ctx, entry, deadline) -> Self { pre }`. It has one `finish(self, post_deadline)`. Its `Drop` emits `pre` with `skipped_post: "not_dispatched"`, which fixes #18. Capture the post right after `execute_single_shot` returns, inside `execute_with_auto_wait_and_lease`, with a detached 1 s budget. Hand the finished outcome up, so helpers.rs no longer calls `finish_artifacts` after the wait (#17). Capture pre once per command, not per iteration. Make the skip reasons an enum (`Disabled | Deadline | NoTarget | NoSession | DispatchBudget | NotDispatched | Dir(String)`). Files: `ref_action/ref_action_wait.rs`, `ref_action/mod.rs`, `commands/helpers.rs`, `trace/trace_artifacts.rs`. Size M (about 150 lines). #18 and #17 can land separately.
- **Overlap**: upstream #17, #18, and S5 (the artifact budget ledger). Perf work on the auto-wait loop touches the same lines.
- **Prior art**: none found beyond PR #91 (the trace viewer that introduced artifacts). Searches: `"post-action screenshot"`, `"artifact screenshot pre post"`, `"screenshot_pre"`; `git log upstream/main -Sfinish_artifacts`.

### ACTION-4: Notification wait baseline: fixed after the first list, and taken after the trigger (#19 and wider)

- **Pieces**
  - `commands/wait_notification.rs:35`: `baseline: Option<HashMap<Fingerprint, usize>>`.
  - :45-48: set once, never updated.
  - :117-131: `first_new_notification` compares counts.
  - `src/batch/execution.rs:29-101`: `pending_baseline` is captured before the preceding entry only for `wait --event` (`event_filter`).
  - `context.event_baseline()` is ignored by `wait_for_notification` (wait.rs:67-69 does not pass it on).
- **States and transitions today**
  ```
  NoBaseline --list Ok--> Baseline(counts at wait start) --list: some count > baseline--> Matched
                                                         --otherwise--> Baseline (unchanged, even when counts drop)
  ```
- **Illegal transitions reachable**
  1. #19: CONFIRMED. The baseline holds {N:1}. The user or the app dismisses N, and the same N arrives again (same app, title, body and actions; for example a repeated "Download complete"). The counts never exceed 1, so the wait times out.
     - Test: in `commands/wait_test_support.rs`, a notifications stub returning `[N]`, `[]`, `[N]` on successive lists. Run `wait --notification --timeout 1000` under `VirtualClock`. Assert `matched: true`. Today it is TIMEOUT.
  2. Baseline taken after the trigger: CONFIRMED. `click @send; wait --notification`, as two CLI calls or as two entries in one batch. A notification that lands before the wait's first list is in the baseline and is never reported. `batch` cannot help, because the pre-action baseline seeding covers `--event` only. This is the same failure shape as `docs/solutions/runtime-errors/a-wait-for-closed-right-after-the-opener-is-satisfied-falsely.md`: the wait is satisfied, or here blinded, by state that the trigger already produced.
     - Test: a batch of `[click, wait --notification]` with a stub that adds N inside `execute_action`. Assert `matched`. Today it is TIMEOUT.
- **Hidden cost**: a full notification list per 100 ms poll (WaitScheduler); nothing extra.
- **Impact class**: B (a false TIMEOUT; the agent may redo the trigger, which sends a second message).
- **Proposed explicit form**: a `NotificationBaseline { seen: HashMap<Fp, usize> }` with `observe(current) -> Option<&Info>`. It matches when any count exceeds `seen`, and it then lowers `seen` to `min(seen, current)` on every list, which turns dismissals into a drop in the count. Teach the batch's `event_filter` to seed a notification baseline the same way it seeds a `SignalBaseline`. Better: one `PreActionBaseline` enum (`Signals(SignalBaseline) | Notifications(...)`) carried in `CommandContext`. Files: `commands/wait_notification.rs`, `src/batch/execution.rs`, `context/mod.rs`. Size S for #19 alone (about 30 lines); M with batch seeding.
- **Overlap**: upstream #19 and SM7; issue #433 and PR #469 for the `--event` analogue.
- **Prior art**: issue #433 (event wait after the action, the same race for events), PR #469 (hint only). Nothing for notifications. Searches: `"notification baseline"`, `"wait notification dismissed"`, `"wait --notification missed"`; `git log upstream/main -Sfirst_new_notification`.

### ACTION-5: `wait` mode is decided by an Option-count over flat flags; orphan options pass silently (issue #605)

- **Pieces**
  - `commands/wait.rs:24-49`: `WaitArgs`, `WaitModeArgs`, `WaitPredicateArgs`. Seven Options plus a `timeout_ms` with a default value.
  - `commands/wait_mode.rs:103-142`: `validate_wait_mode` counts selected modes. `--window` counts as a mode only without `--event`, and `--text` only without `--notification`.
  - `commands/wait_mode.rs:39-96`: `from_args`, which chooses by early-return order.
  - `src/cli_args/system.rs:213-232`: `--timeout` has `default_value = "30000"`. The help for `--app` says "Scope element, window, or text wait".
- **States and transitions today**: the mode is whichever early return fires first. Options outside the chosen mode are dropped unless one of four ad hoc checks catches them (`--predicate`, `--value`, `--count`, event window filters).
- **Illegal transitions reachable**
  1. #605: CONFIRMED. `wait 0 --action type` returns ok, and so does `wait 0 --window-id w1`. Open PR #606 adds checks for exactly these two.
  2. Still open after #606, CONFIRMED: `wait --element @s:e1 --app Safari` ignores `--app` (`WaitMode::Element` has no app field, wait_mode.rs:73-81), although the help promises scoping. `wait 500 --app X` ignores `--app`. `wait 500 --timeout 100` ignores `--timeout`, and an orphan `--timeout` cannot be detected at all, because clap fills in 30000.
     - Test: `validate_wait_mode` with `ms: Some(0), app: Some("X")` expects `INVALID_ARGS`; today it returns Ok.
- **Hidden cost**: none.
- **Impact class**: B (a wrong success: the caller believes a scope was applied).
- **Proposed explicit form**: parse straight into `enum WaitMode` with per-variant structs. Clap `ArgGroup`s or `subcommand`-like groups make orphans unrepresentable. In core, `WaitMode::from_args` returns `INVALID_ARGS` for any populated field the chosen variant does not consume, generated from one table of `(field, allowed_modes)`. Make `timeout` an `Option<u64>` at the CLI layer so an orphan is detectable. Files: `commands/wait_mode.rs`, `commands/wait.rs`, `src/cli_args/system.rs`, and the batch and MCP schemas. Size M. The PR #606 subset is S.
- **Overlap**: issue #605 / PR #606.
- **Prior art**: issue #605, PR #606 (open; partial). Searches: `"wait action window-id"`, `"orphan option wait"`, `"wait --app ignored"`; `git log upstream/main -Svalidate_wait_mode`.

### ACTION-6: Delivery disposition is a last-writer-wins field with string round trips, not a lattice

- **Pieces**
  - `response/adapter_error.rs:50-53`: `with_disposition` overwrites unconditionally.
  - :85-90: `unsent()` writes only over `Unknown`.
  - `ref_action/mod.rs:188-197`: `mark_pre_dispatch_resolution_failure`, upgrade-only.
  - `ref_action/ref_action_wait_support.rs:133-142`: `after_scroll`, `Unknown` only.
  - `commands/pointer_action.rs:151-160`: `before_first_event` forces `not_delivered` over any value.
  - `ref_action/mod.rs:238-243`: `trace_error_after_delivery` forces `delivered_unverified`, even over a `NotDelivered` "already satisfied" result.
  - `actions/post_action.rs:234-243`: `verification_error` maps everything except `NotDelivered` to `delivered_unverified`.
  - `commands/operation_response.rs:28-38`: `delivery_in` parses the disposition back out of result JSON and folds `Unknown` into `delivered_unverified`.
  - `context/mod.rs:18-30,315-327`: `CommandScope::complete` re-derives the disposition from `value["disposition"]` with a fallback.
  - `src/dispatch/disposition.rs:16-23`: `settle_unknown_delivery`.
  - `actions/action_result.rs:82-94`: `with_verified_delivery` and `with_unverified_delivery`, guarded transitions.
- **States and transitions today**: the enum is explicit (`actions/delivery_semantics.rs:5-13`), but the transitions are per-call-site policies. Some upgrade only, some downgrade only, and some force a value. Two of them re-parse JSON.
- **Illegal transitions reachable**: none confirmed that produces a double delivery today, apart from ACTION-1 and ACTION-2. Structural risk, SUSPECTED: `before_first_event` would wrongly mark a drag `not_delivered` as soon as any pointer event moves into its closure, because nothing enforces "no input inside". `trace_error_after_delivery` turns an already-satisfied `NotDelivered` toggle into `delivered_unverified`. That direction is safe, but it reports work that never happened.
- **Hidden cost**: `serde_json` round trips of the result in `delivery_in` and `result_disposition`, which is negligible.
- **Impact class**: D (clarity). The fix also prevents a future A.
- **Proposed explicit form**: `DeliverySemantics::join(self, evidence) -> Self`, a monotone join where any delivered or uncertain value beats `NotDelivered`, which beats `Unknown`. Add a phase typestate: `PreDispatch` errors may only `join(NotDelivered)`, and `PostDispatch` errors may only move up. Replace `with_disposition` at the call sites listed above with `join` or with `pre_dispatch` from ACTION-1. Carry the disposition as a typed field next to the JSON instead of re-parsing it. Files: `response/adapter_error.rs`, `actions/delivery_semantics.rs`, and the roughly 8 call sites above. Size M. This is a pure refactor except for the `trace_error_after_delivery` nuance.
- **Overlap**: upstream SM4 in "Ref actions and waits"; the CLI/batch area owns `src/dispatch/disposition.rs`.

### ACTION-7: Stability evidence is not handed from the read-only poll to the dispatch preflight

- **Pieces**
  - `ref_action/ref_action_poll.rs:33-40`: `expected_bounds_hash` comes from the entry and is dropped for headed actions (`.filter(|_| policy == headless)`).
  - `ref_action/ref_action_poll_state.rs:17-31`: `strict_hash`; `record_preflight_error` adopts `observed_bounds_hash` parsed from error JSON (`ref_action_wait_evidence.rs:46-52`).
  - `ref_action/mod.rs:263-304`: `stable_preflight` restarts from `permissive(entry.geometry.bounds_hash)`, the snapshot hash and not the hash the poll learned, then runs `StabilitySampler`.
  - `actionability/stability.rs:1-40`: `StabilityExpectation { bounds_hash, bounds, strict, samples, span_ms }` encodes three modes.
  - `actionability/gates.rs:82-131`.
  - Constants: `STABILITY_POLL_INTERVAL 16 ms` (poll.rs:17), `STABILITY_SAMPLE_INTERVAL 17 ms`, `MIN_STABILITY_SPAN 34 ms`, 3 samples (stability_sampler.rs:4-5,40).
- **States and transitions today**
  ```
  Poll: Unknown(hash None|entry) --preflight fail "stable"--> Adopted(observed) --16 ms--> Pass   (headed: always 2 polls)
  Dispatch: Permissive(entry hash) --mismatch--> Sampling{prev, consecutive, since} --3 samples >= 34 ms--> Strict(bounds) --> check again
  ```
  The poll's `Adopted(hash)` is discarded at the lease boundary.
- **Illegal transitions reachable**: none for correctness, because the dispatch re-check is the safety boundary, by design (poll.rs:19-27). Upstream issue #155 (closed) fixed the old fail-closed version of this path.
- **Hidden cost**: measured as read counts in tests, latency estimated.
  - Headed ref action: always 2 read-only resolves and 2 live preflights, plus a 16 ms sleep (`ref_action_wait_lease_tests.rs:128-145` asserts `read_only_resolve_attempts == 2`, `live_calls == 3`).
  - Any action whose element moved since the snapshot (after `scroll-to`, a window move or a reflow): the poll pays one extra iteration (16-100 ms, plus a resolve and a live read). Dispatch then pays ≥3 `get_element_bounds` reads, ≥34 ms (in practice about 51 ms with three 17 ms sleeps), and one more full live check with a hit test (`a_stability_only_mismatch_settles_and_proceeds_to_dispatch` asserts `bounds_reads >= 3`).
  - Estimate: 50-120 ms and 5-6 extra platform round trips per moved-element action.
  - Also `presentation::target_center` (presentation.rs:71-97) does an extra `land_saved_ref` plus a bounds read before the lease when the stored bounds are window-space (Linux) and the overlay is on.
  - `effect_ripples` adds a 150 ms-capped `get_live_state` for TypeText with the overlay on (presentation.rs:108-127).
- **Impact class**: C.
- **Proposed explicit form**: `enum StabilityExpectation { Permissive { hash: Option<u64> }, StrictHash { hash: Option<u64> }, Sampled { bounds: Rect, samples: u32, span_ms: u64 } }` makes the five-field combinations unrepresentable. `execute_poll_loop` returns `RefActionPollState`. Pass `state.expected_bounds_hash` into `RefActionContext`, and have `stable_preflight` start from `StrictHash(adopted)` when the poll adopted one; it falls back to sampling only on a mismatch. Files: `actionability/stability.rs`, `actionability/gates.rs`, `ref_action/mod.rs`, `ref_action/ref_action_poll.rs`, `ref_action/ref_action_context.rs`. Size M. The enum refactor (S) can land separately from the handoff (S, behavior plus perf).
- **Overlap**: issue #155; any perf/latency branch on the ref-action hot path (`ref_action/mod.rs`, `ref_action_poll.rs`).
- **Prior art**: issue #155 (closed, the fail-closed predecessor). Nothing for the handoff. Searches: `"stable_preflight"`, `"stability sampler latency"`, `"bounds hash moved snapshot"`; `git log upstream/main -Sexpected_bounds_hash`.

### ACTION-8: Post-action `--wait-for` deadline handoff and refstore saves (#10 reaches waits)

- **Pieces**
  - `commands/helpers.rs:165-189`: the post-action wait gets its own `Deadline::at(start, wait.timeout_ms)` (`wait_selector.rs:46`), bounded only by the inherited batch deadline.
  - `wait_selector.rs:294-306`: `materialized_response` calls `save_refmap`. A complete snapshot takes `save_new_snapshot`, whose `RefStoreLock::acquire` uses `Deadline::after(2000)` min inherited (`refs/refs_lock.rs:12-15`). An incomplete one takes `save_new_snapshot_detached` (`snapshot/mod.rs:28-34`).
  - `wait_selector.rs:172-210`: the timeout path builds a 600 ms *detached* diagnostic snapshot and saves it detached (#154's fix).
  - `storage/file_lock.rs:101-104`: an expired deadline fails before the first `try_lock`.
- **States and transitions today**
  ```
  WaitPolling --match, complete snapshot--> SaveUnderInheritedDeadline --inherited expired--> TIMEOUT(lock_timeout)   <- satisfied wait reported as failure
              --match, incomplete snapshot--> SaveDetached --> Ok
              --deadline--> DiagnosticBuild(600 ms detached) --> SaveDetached(2 s) --> wait_timeout
  ```
- **Illegal transitions reachable**
  1. CONFIRMED by code, needs a batch: `batch '[{"click":...,"wait_for":"button:Done"}]'` with a batch deadline that expires between the match and the save. The selector matched, yet the entry fails `TIMEOUT kind: lock_timeout` (disposition copied from the delivered click, so retry is unsafe, which is correct). A wait that did not match would instead have saved its snapshot and reported `wait_timeout`. This is #10's asymmetry, now on the success path. #154 fixed only the failure path.
     - Test: `enter_scope(Some(deadline))` with a deadline that `VirtualClock` expires inside the adapter's `get_tree` for the materialization build. Assert `Ok` with `snapshot_id`. Today it is `TIMEOUT` with `kind: lock_timeout`.
  2. The detached diagnostic build and lock on the timeout path can overrun the inherited batch deadline by up to 600 ms + 2 s. CONFIRMED by code; documented as intended recovery.
  3. Minor inconsistency, CONFIRMED: `wait --element` rejects a predicate that was satisfied by an observation that finished after the deadline (`commands/wait_element.rs:48-64`: the expired check runs before `satisfied`). Window, text and selector waits accept a late match (wait.rs:152-166 in `wait_for_window`, wait_selector.rs:59-62). This may be intentional.
- **Hidden cost**: a timed-out post-action wait costs `wait_timeout + up to 600 ms (diagnostic build) + up to 2 s (lock)`. The near-deadline build at wait_selector.rs:111-120 adds one full snapshot in the last 100 ms window.
- **Impact class**: B.
- **Proposed explicit form**: one `PersistPolicy` decided once per observation: `Recovery` (detached) for any already-built result after the command deadline, whether complete or not; `Live` otherwise. `SnapshotResult::save_refmap(store, policy)` takes it explicitly instead of inferring it from `complete`. This is the policy decision #10 asks for. Files: `snapshot/mod.rs`, `commands/wait_selector.rs`, `commands/snapshot.rs`, `refs/refs_store.rs`. Size S.
- **Overlap**: upstream #10; issue #154 (fixed, failure path only); PR #372 (introduced the detached incomplete save).
- **Prior art**: issue #154, PR #164, PR #372. Searches: `"save_refmap deadline"`, `"complete snapshot timeout inherited"`, `"wait-for batch timeout snapshot"`; `git log upstream/main -Ssave_new_snapshot_detached`.

### ACTION-9: Auto-wait outer loop and poll state: duplicated flags, per-iteration resets, repeated side effects

- **Pieces**
  - `ref_action/ref_action_wait.rs:80-134`: `polls: bool` is derived from `request.timeout_ms`; the loop-local `saw_ambiguity` and `refusal: Option<AdapterError>`; a fresh `RefActionPollState` per iteration (:84-89); `request.timeout_ms` rewritten to `remaining_ms` (:107-109). It doubles as the "may wait for geometry" flag in `actionability/evidence.rs:20`.
  - `ref_action/ref_action_poll_state.rs:5-14`: `last_report: Option<Value>` with a `"phase"` string; `last_settled_identity_error`; `live_read_incomplete_only: Option<bool>`; counters.
  - `ref_action/ref_action_poll.rs:109-119`: `StaleRef` with `complete:true` is terminal before `record_resolve_error`, so the `StaleRef` arm of `is_settled_identity` (poll_state.rs:118-126) is unreachable from the loop. It is covered only by direct-state tests (`ref_action_poll_tests.rs:39-56`).
  - `ref_action/ref_action_single.rs:29-52`: the headed focus runs on every iteration, before the dispatch-time resolve.
  - `ref_action/mod.rs:100-122`: scroll recovery runs on every iteration.
- **States and transitions today**: as upstream SM1 and SM2. Additional facts:
  - (a) `auto_wait.read_only_resolve_attempts` and `read_only_preflight_attempts` describe only the last iteration, because the state is rebuilt after each refusal.
  - (b) `saw_ambiguity` lives in two places and is OR-merged (:81, :90, :127).
  - (c) After each refusal, the next iteration raises the target window again (`focus_app` or `focus_entry_window`) and may scroll again before it learns whether the element resolves.
- **Illegal transitions reachable**
  - (a) CONFIRMED: misleading metrics.
  - (c) CONFIRMED: a repeated window raise or scroll per refusal. This is not a repeated action. On a retry-heavy target, though, focus is stolen up to `timeout / 100 ms` times.
  - Test sketch for (a): use the adapter in `ref_action/ref_action_wait_dispatch_tests.rs` that refuses once under the lease. Assert `auto_wait.read_only_resolve_attempts` equals the total across both iterations. Today it equals the second iteration only.
- **Hidden cost**: per refusal, a full poll restart (≥1 resolve plus a live read), a 100 ms sleep (`refs::ref_resolve_deadline::POLL_INTERVAL`), a cursor hold, pre-lease travel, a lease round trip, focus (headed) and an artifact pair (ACTION-3).
- **Impact class**: C for the repeated focus and the cost; D for the flags and metrics.
- **Proposed explicit form**: `enum AutoWait { Single, Polling { budget: Deadline } }` replaces `polls` plus the `timeout_ms` overload. Use one `AutoWaitState { poll: RefActionPollState, refusal: Option<Refusal>, totals: Counters }` that persists across iterations. Move `saw_ambiguity` into it. Make `last_report` an enum `{ Preflight(Value), Resolve { code, message, details } }`. Fold `live_read_incomplete_only` into `enum TimeoutCause { None, LiveReadIncompleteOnly, Mixed }`. Skip focus on iteration n > 0 when the window is still frontmost, or resolve first and focus after. Files: `ref_action/ref_action_wait.rs`, `ref_action_poll.rs`, `ref_action_poll_state.rs`, `ref_action_single.rs`. Size M, a pure refactor except the focus order. The focus order is a behavior change; split it and probe it on macOS.
- **Overlap**: upstream SM1, SM2 and SM5 (TRAVELLED_BEFORE_LEASE is reset per iteration and never stale; I verified there is no illegal state there).
- **Prior art**: none found. Searches: `"auto_wait metrics"`, `"read_only_resolve_attempts"`, `"refocus retry headed"`, `"focus_app retry"`.

### ACTION-10: Post-action verification settle loop (`execute_verified_action`)

- **Pieces**
  - `actions/post_action.rs:7-139`: `before` read (TypeText and Toggle) and `get_text_selection` (TypeText).
  - Toggle is rewritten to Check or Uncheck from the `before` read (:21-29).
  - Settle loop: `settle = 50 ms`, `settle_deadline = deadline.capped(400 ms)`, extended to the full lease deadline while a keyed value keeps changing (`drains && changing`, :55-74).
  - `verification_error` (:234-262).
- **States and transitions today**: `NotStateful → execute`. `Stateful → [before] → execute → Err(not_delivered): return | else observe → (mismatch && budget) sleep 50 ms → observe … → Verified | Unverified | Mismatch(Err delivered_unverified)`.
- **Illegal transitions reachable**: none of class A. Toggle race, SUSPECTED: `before` reads unchecked, another actor checks it, the request becomes `Check`, and the adapter returns `satisfied_without_delivery`. The postcondition `before != after` is true, so the result is `Ok{action: "toggle", disposition: not_delivered, verification_scope: "element_state"}`: a verified toggle that toggled nothing.
- **Hidden cost (estimate)**: every stateful action pays 1-2 reads before (`get_live_element`, plus `get_text_selection` for type) and ≥1 read after. A value the app normalizes (phone or date masks, trimming) never matches, so the command spends the full 400 ms (8 reads) and then fails `ACTION_FAILED` with delivered_unverified. Physical typing into a field that keeps changing can extend to the whole lease deadline. Then `settled_surfaces` (mod.rs:161-163, `list_surfaces` for surface-raising actions), the caret read (presentation.rs:199-206), the post screenshot and any `--wait-for` poll read the same element again.
- **Impact class**: C.
- **Proposed explicit form**: an explicit `Verification { NotApplicable, Pending { expected }, Verified, Unverified(reason), Contradicted }` with one `settle(observe) -> Verification`. Expose the settle budget as a constant next to the other wait constants. Share the final `observed` with `settled_surfaces` and the caret placement so they don't re-read. Size S-M.
- **Overlap**: PR #190 (introduced verification); any perf work on post-action reads.
- **Prior art**: PR #190. Nothing on settle cost. Searches: `"post-action verification settle"`, `"execute_verified_action"`.

### ACTION-11: Wait-side stringly or JSON-typed state (short entry)

- `commands/post_action_observation.rs:9-15,41-110`: `Probe.outcome: &'static str` plus Option fields, and the string comparison `latest.outcome != "owner_unavailable"` (:107). No reachable illegal value, matching upstream SM8.
- `commands/wait_element.rs:32-63`: `last_observed: Value` doubles as an observation or an error, and `expected_bounds_hash` is mined from it.
- `commands/wait_scheduler.rs:22-35` and `wait_element.rs:98`: `std::thread::sleep` instead of `wait::deadline::sleep`, which bypasses `VirtualClock` in tests. Also `wait.rs:60` (Sleep mode) and `type_text_interval.rs:124`.
- `ref_action/ref_action_wait_evidence.rs:4-44`: control flow decided by scanning `details.checks[].check == "visible"` strings.
- Impact D. Proposed form: enums for `ProbeOutcome` and `ElementObservation`, and a typed `ActionabilityReport` passed alongside the error instead of parsing JSON. Size S each.

### Verified, no finding

- `ref_action/presentation.rs` `TRAVELLED_BEFORE_LEASE`: reset on every `before_lease` and taken in `after_dispatch`. A failure in between leaves a stale `Some` only until the next ref action's `before_lease`, and `before_dispatch` has no production caller that skips `before_lease`. This matches upstream SM5.
- `wait/deadline.rs` inherited-deadline scope: `enter_scope` and `Drop` restore it correctly. `Deadline::after`, `at` and `extended_by` all apply the min with the inherited deadline. `detached_after` is the only escape, and it is used for recovery saves and diagnostic builds (ACTION-8).
- `commands/helpers.rs:216-239` `finish_post_action_wait`: a failed post-action wait copies the action's delivery (`delivery_in`), so a timed-out `--wait-for` never claims `not_delivered` after a delivered action.
- `ref_action_wait.rs:140-144` `resolves_again`: re-dispatch requires `NotDelivered` plus a retryable resolution code. Core never stamps `NotDelivered` after `execute_verified_action` except when the adapter itself returned `NotDelivered`. Whether each adapter honors that is out of scope here, so a PLATFORM agent should check adapter `NotDelivered` after partial input.
