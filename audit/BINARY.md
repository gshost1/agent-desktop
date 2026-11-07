# BINARY area: implicit state machines in the `agent-desktop` binary crate (`src/`)

Repo `/home/george/Projects/agent-desktop`, main = `b43ac4f6` (= `upstream/main`, no commits since). Read-only audit; no
cargo runs. Paths are relative to the repo root. "CONFIRMED" = traced end to end in source; "SUSPECTED" = needs a probe.

Upstream register status for this area (checked against main):
- #5 (MCP cancel frees id): still present on main (`src/mcp/calls.rs:51-61`); fixed on `upstream/laf/state-machine-fixes`
  (1170bcbc, PR #602) with a `Slot {Admitted, Running, Cancelled}` enum. Skipped as instructed.
- #11 (visual debug create-once under `mcp`): STILL TRUE on main, and wider than the register says. See BINARY-9.
- #12 (`session start --cursor` drops the session id on failure): STILL TRUE. See BINARY-10.
- #13 (overlay Hide/Show is a switch, not a count): STILL TRUE, and a batch reaches it deterministically in-process. See BINARY-11.
- #605 (wait `--action`/`--window-id` without their mode): only overlap is `command_policy::validate_args` for `Wait`
  (`src/command_policy/mod.rs:193-197`), which checks only the element ref. The ACTION agent owns it.

Summary table

| id | name | impact | status |
|---|---|---|---|
| BINARY-1 | Oversized batch entry gets `INVALID_ARGS` + `delivery_uncertain` for a read-only result | B | CONFIRMED |
| BINARY-2 | Batch run state: wait-event baseline keyed on adjacency; stop reason precedence | B | CONFIRMED (adjacency, precedence), SUSPECTED (wait-after-wait) |
| BINARY-3 | Batch `session_ended` gate refuses repeatable `session end` that standalone allows | B (low) | CONFIRMED |
| BINARY-4 | Strict-trace failure at batch `command.end` replaces the whole batch body | B | CONFIRMED |
| BINARY-5 | `is_mutating` overload: local-write commands keep `delivery: unknown` on read failures | B | CONFIRMED |
| BINARY-6 | `settle` infers "sent nothing" from `INVALID_ARGS` code | B (narrow) | SUSPECTED |
| BINARY-7 | `pre_dispatch_error` collapses non-adapter errors to `INTERNAL` (drops `PERM_DENIED`) | B (low) | CONFIRMED mapping, SUSPECTED producer |
| BINARY-8 | Permission report runs on a fixed 5 s deadline outside the command/batch budget; dead deadline/baseline params | C | SUSPECTED (Linux silent bus) |
| BINARY-9 | Visual debug artifact create-once + per-command globals forwarded to every MCP tool (#11) | B | CONFIRMED |
| BINARY-10 | `session start --cursor` two-step persistence (#12) | B (low) | CONFIRMED |
| BINARY-11 | Dispatch overlay Hide/Show bracket (#13) and its in-process batch nesting | D (C cost) | CONFIRMED |
| BINARY-12 | MCP cancel kills the CLI mid-input on macOS/Windows with no release | A | SUSPECTED |
| BINARY-13 | MCP call registry / pool / EOF lifecycle (beyond #5) | D | CONFIRMED |
| BINARY-14 | Batch item contexts frozen at prepare time | D | SUSPECTED (cross-process only) |

---

### BINARY-1: Oversized batch entry disposition falls back to `delivery_uncertain`
- Pieces: `src/batch/execution.rs:103-116` (entry built, `failed` computed from the unbounded entry, then bounded),
  `src/batch/result_entry.rs:128-154` (`bounded_completed_entry`), `:198-211` (`entry_disposition`: `data.disposition`,
  else `error.disposition`, else `DeliverySemantics::uncertain()`), `src/batch/bounded_json.rs:216-224`.
- States today:
  `Completed(ok, data)` --fits--> kept. `Completed(ok|err)` --exceeds remaining budget--> `Completed(err INVALID_ARGS,
  result_omitted, disposition = data.disposition ?? error.disposition ?? uncertain)` and `stopped = output_limit`.
- Illegal transitions reachable:
  1. A read-only entry (snapshot, inline screenshot, trace show, skills get --full, clipboard-get) has no
     `data.disposition`, so its omitted result is reported `delivery: delivery_uncertain`, `retry: unsafe`. The
     command sent nothing; `dispatch::disposition::settle` would have stamped any failure of it `not_delivered`
     (`src/dispatch/disposition.rs:17-24`). The caller is told a re-run of a read is unsafe and gets no `recovery`.
     Path: `execution.rs:106` dispatch Ok -> `:109` `bounded_completed_entry` -> `result_entry.rs:140` ->
     `:210` fallback. Input: an inline `screenshot` entry of a large display (base64 PNG easily > 4 MiB - 8 KiB), or
     several medium screenshots whose sum crosses the budget. CONFIRMED (traced end to end).
  2. The code is `INVALID_ARGS` even when the action was delivered (`delivered_verified` copied from data). A caller
     keying on code reads "your arguments were wrong" for a delivered click. CONFIRMED.
- Test sketch (fails on main): in `src/batch/execution_tests.rs`, copy the existing oversized case (line ~203) but
  drop the `"disposition"` key: `bounded_entry(0, "snapshot", Ok(json!({"payload": "x".repeat(MAX_BATCH_OUTPUT_BYTES)})), 0)`
  and assert `entry["error"]["disposition"]["delivery"] == "not_delivered"` and `retry == "safe"`. Main returns
  `delivery_uncertain`/`unsafe`.
- Hidden cost: none beyond the two serialization passes per entry (`completed_entry` then `serialized_fits`), estimate
  negligible.
- Impact: B.
- Proposed explicit form: carry the command's effect class into the batch entry (`EntryOutcome { effect: CommandEffect,
  result }`, see BINARY-5) and derive the omitted-result disposition from it: `ReadOnly | LocalWrite -> not_delivered`,
  `AppInput -> data.disposition ?? uncertain`. Use a distinct code or `details.kind = "batch_output_limit"` instead of
  bare `INVALID_ARGS`. Files: `src/batch/result_entry.rs`, `src/batch/execution.rs`. Size S. Behavior fix only.
- Overlap: none in the upstream register. Same files as BINARY-2.
- Prior art: none found. Searches: issues+PRs on vercel-labs and gshost1 for "batch output limit disposition",
  "result_omitted", "batch entry exceeded response limit"; `git log upstream/main -S entry_disposition -- src` (only the
  original 3f322728).

### BINARY-2: Batch run state: pending wait baseline keyed on adjacency, stop-reason precedence
- Pieces: `src/batch/execution.rs:21-31` (deadline, `pending_baseline` seeded from `context.event_baseline()`,
  `stopped`, `completed`), `:33-43` deadline pre-check, `:45` take, `:47-59` session_ended skip, `:61-95` capture for
  `commands[index + 1]` only, `:97-101` item context, `:118-129` stop precedence, `:132-155` body; `:177-185`
  `event_filter`. Run-level state is four loose locals (`results`, `completed`, `pending_baseline:
  Option<Result<SignalBaseline, AdapterError>>`, `stopped: Option<Value>`), and `stopped` is a hand-built JSON value with
  two shapes for one reason (`{"reason":"deadline","index"}` at :41 vs `{"reason":"deadline","after_index"}` at :127).
- States and transitions today:
  ```
  per entry i:  Pending --deadline expired--> NotStarted(deadline), STOP(deadline,index)
                Pending --session ended-----> NotStarted(session_ended), STOP(stop_on_error) iff flag else continue
                Pending --entry i+1 is wait --event: capture(filter of i+1)--Err--> NotStarted(pre_action_baseline_failed), STOP
                Pending --dispatch--> Completed --oversized--> STOP(output_limit)
                                                 --failed && flag--> STOP(stop_on_error,index)
                                                 --deadline expired && i+1<total--> STOP(deadline,after_index)
  ```
- Illegal / surprising transitions reachable:
  1. Baseline taken after the action when anything sits between them. The capture for a `wait --event` at j happens
     only at iteration j-1 (`execution.rs:61`). `[click, screenshot, wait --event window-opened]` captures at the
     screenshot, i.e. after the click, so a window the click opened before the capture is in the baseline and the wait
     times out (or is satisfied only by a later, unrelated window). The skill states the rule only for an adjacent
     "action-then-event sequence" (`skills/agent-desktop/references/commands-system.md:168`); the code silently moves the
     baseline otherwise. CONFIRMED (traced: `commands.get(index + 1).and_then(event_filter)`).
  2. Wait-after-wait double counting. For `[click, wait --event window-opened, wait --event window-opened]`, wait #2's
     baseline is captured before wait #1 runs. A window W that appears after that capture satisfies both waits.
     SUSPECTED (timing race in practice; deterministic with a stub).
  3. Stop-reason precedence: an entry that fails with `TIMEOUT` because the batch deadline expired under it reports
     `stopped.reason = "stop_on_error"` with `--stop-on-error`, and `"deadline"` without it (`:122-129` order). A caller
     deciding whether to raise `--timeout-ms` sees a different reason for the same event. CONFIRMED.
  4. Dead path: `pending_baseline` is seeded from the batch's own `context.event_baseline()` (`:29-30`), which is always
     `None` (main passes `baseline: None`, `src/main.rs:147`, and batches cannot nest). The `Err` arm of the
     `Option<Result<..>>` is never constructed (an `Err` breaks the loop at `:64-91`). D.
- Test sketch (fails on main) for 1: stub adapter whose `capture_signal_baseline` records the call order and returns an
  empty baseline, and whose screenshot/click succeed; run `execution::execute` with
  `[{"command":"mouse-click",...},{"command":"list-displays"},{"command":"wait","args":{"event":"window-opened","timeout":50}}]`;
  assert the capture happened before entry 0 dispatched (record a counter in both). On main the capture happens at
  iteration 1. For 3: `RecordingReadAdapter`-style stub where `list_displays` returns `deadline.timeout_error()`, batch
  of two entries with `timeout_ms: 25` and `--stop-on-error`; assert `stopped.reason == "deadline"`.
- Hidden cost: one `capture_signal_baseline` per wait entry (a full app/window/surface inventory, platform round
  trips), charged to the batch deadline before entry i. Estimate tens of ms on macOS; not measured.
- Impact: B.
- Proposed explicit form: `enum BatchStop { Deadline { before: usize } | Deadline { after: usize } | StopOnError {..} |
  BaselineFailed {..} | OutputLimit {..} }` with one serializer; precedence decided in one `fn next_stop(&EntryOutcome,
  &Deadline, flags) -> Option<BatchStop>` that prefers `Deadline` when the entry error is the batch deadline. Baseline:
  `struct PendingBaseline { for_index: usize, baseline: SignalBaseline }` captured before the last app-input entry that
  precedes each `wait --event` (scan once at prepare time into `Vec<Option<usize>> capture_before`), handed only to
  the entry whose index matches. Makes "baseline for the wrong entry" and "Err baseline in flight" unrepresentable.
  Files: `src/batch/execution.rs` (split into `stop.rs`, `baseline_plan.rs`), tests. Size M. The stop-precedence fix is
  S and separable; the baseline-placement change is a behavior change that needs a skill update.
- Overlap: upstream register CLI SM3 (described, no finding). Merged #182 (`fix: skip ended-session batch entries before
  wait baseline pre-capture`, issue #177) touched the same loop. Open #433 (`wait --event window-opened after the action
  times out with a misleading suggestion`) is the standalone symptom of a late baseline.
- Prior art: #177 (closed) / PR #182 (merged) are the nearest; neither covers non-adjacent placement or precedence.
  Searches: "batch stop_on_error deadline stopped reason", "batch wait event baseline", "batch timeout_ms" (issues+PRs,
  both repos).

### BINARY-3: Batch `session_ended` gate refuses the repeatable `session end` standalone allows
- Pieces: `src/batch/execution.rs:47-59, 214-220` (`session_ended_for` runs before every entry, any command);
  `src/dispatch/mod.rs:58-61` (standalone exempts `session end` via `session_end_scope`);
  `crates/core/src/context/session_scope.rs:34-42` ("Allows repeatable session teardown").
- States: session `Active | Ended`; per command `Allowed | Refused`. Standalone: `session end` is allowed on `Ended`
  (re-runs the overlay `Disable` teardown, `src/dispatch/session.rs:171-175`). Batch: refused as
  `not_started(session_ended)`.
- Illegal transition: retrying a failed cursor teardown inside a batch. `session end` returns
  `"Session state was saved, but cursor teardown was not confirmed"` (`src/dispatch/cursor_overlay.rs:91-99`) when the
  renderer did not ack; the standalone retry works, the batch retry (`[{"command":"session","session":S,"args":{"action":"end"}}]`)
  is skipped. Also `version`/`skills` entries in an ended session are skipped although standalone `--session S version`
  succeeds (main handles them before any context, `src/main.rs:162-178`); the test at `src/tests/cli_process.rs:323-330`
  pins that as intended, so only `session end` is the inconsistency. CONFIRMED (traced).
- Test sketch: in `src/tests/cli_process.rs` style: `start_session_in_dir`, then `run_batch` with two identical
  `session end` entries for that session; assert `results[1].execution == "completed"` and `ok == true`. Main:
  `not_started` / `session_ended`.
- Hidden cost: `session_ended_for` reads the manifest, then the entry's scope reads it again in `ensure_active`
  (`crates/core/src/context/session_scope.rs:17-19`): 2 manifest reads per entry, plus 1 at prepare. Estimate <0.2 ms
  per entry. Measured: no.
- Impact: B (low).
- Proposed explicit form: one predicate shared by dispatch and batch: `fn session_requirement(cmd: &Commands) ->
  SessionRequirement { Active, AllowEnded, None }`; batch skips only `Active` entries in an ended session. Files:
  `src/cli/mod.rs` (metadata), `src/dispatch/mod.rs`, `src/batch/execution.rs`. Size S.
- Overlap: #1/#8 (session end ordering) on the fix branch touch `end_session`, not this gate.
- Prior art: #177 / PR #182 introduced the gate; nothing on the `session end` exemption. Searches: "batch session end
  ended session", "session_ended batch"; `git log upstream/main -S session_ended_for -- src` (only 922bb7d5).

### BINARY-4: Strict-trace failure at `command.end` replaces a whole batch body
- Pieces: `src/dispatch/mod.rs:58-66` (batch takes `mutating_command_scope`), `:141` `scope.complete(&result)?` returns
  the trace error and drops `result`; `crates/core/src/context/mod.rs:47-59` (`complete`), `:315-328`
  (`result_disposition`: body has no top-level `disposition`, so the success fallback `delivered_unverified` is used);
  `crates/core/src/trace/mod.rs:86-116` (strict emit errors); `crates/core/src/trace/trace_write_budget.rs:57-66`
  (budget exhaustion is an `INVALID_ARGS` error).
- States: batch `Running -> BodyBuilt -> CommandEndEmitted -> Returned`. With `--trace-strict`, `CommandEndEmitted`
  can fail and the transition goes `BodyBuilt -> Err(INVALID_ARGS "Session trace reached its reader-compatible
  recording limit", delivered_unverified, retry unsafe)`; every per-entry result, including which entries delivered,
  is discarded.
- Illegal transition: the caller loses the per-entry map exactly when the batch ran long enough to exhaust the trace
  budget (100 000 events / 32 MiB per session) or the trace file write failed (disk full). It cannot tell which of up
  to 64 entries delivered, and the single disposition says "delivered, retry unsafe" even if every entry was read-only
  or not started. Standalone commands lose less: their disposition is computed from their own data. CONFIRMED (traced).
- Test sketch: CLI-process test with `AGENT_DESKTOP_HOME` temp, start a traced session, pre-write
  `sessions/<id>/trace/.trace-usage.json` as `[0, N, 1]` (the ledger is the `(bytes, events, entries)` tuple,
  `trace_write_budget.rs:51-70`; pick N so the meta line and the batch's own `command.start` still fit under
  `MAX_SESSION_EVENTS = 100_000` and the entry's events do not, e.g. 99 998), run
  `--session <id> --trace-strict batch '[{"command":"version","args":{}},{"command":"version","args":{}}]'`; assert
  `ok == true` and `data.results` present (entry errors allowed). Main: `ok: false`, `INVALID_ARGS`, no results.
- Hidden cost: none.
- Impact: B.
- Proposed explicit form: `CommandScope::complete` returns `TraceOutcome { Recorded | Failed(AdapterError) }` and the
  bracket decides per command: for batch, attach the trace failure to the body (`data.trace_error`) and keep `ok`;
  for single commands keep today's replacement. Files: `crates/core/src/context/mod.rs` (core, owned elsewhere),
  `src/dispatch/mod.rs`. Size S.
- Overlap: register trace S4 (budget ledger) and #4 (ledger emptied on crash, fixed on branch) feed the same failure.
- Prior art: none found. Searches: "trace-strict batch", "strict trace command.end", "strict trace" (issues+PRs, both
  repos); #600 is the ledger-crash case only.

### BINARY-5: `is_mutating` overload leaves `delivery: unknown` on local-write commands
- Pieces: `src/cli/mod.rs:262-330` (`is_mutating`: `Screenshot` iff `output_path` :305, `ClipboardGet` iff `--out`
  :306, `Permissions` iff `--request` :307, `Session` non-list :308-310, `Trace export` :312-313, `Batch` :304);
  `src/dispatch/mod.rs:38-41` (`read_only = !is_mutating`), `src/dispatch/disposition.rs:9-24` (only read-only or
  `INVALID_ARGS` errors are settled to `not_delivered`); `crates/core/src/response/adapter_error.rs:19-29` (new errors
  default to `unknown`); `crates/core/src/commands/screenshot.rs:19-26`.
- States: one boolean stands for three effect classes: `ReadOnly` (no effect), `LocalWrite` (writes a file or local
  state, sends nothing to an app), `AppInput` (may deliver to the app). The boolean drives four decisions: overlay
  Hide/Show (`dispatch/mod.rs:52-57`), scope success disposition (`:62-63`), error settlement (`:38-41`), and the
  multi-agent `--agent-id` check (`src/command_policy/mod.rs:114-119`).
- Illegal transition: the same failure gets two dispositions depending on an unrelated flag. `screenshot --app X` that
  times out resolving X is `not_delivered / safe` with a `recovery` hint; `screenshot --app X out.png` is
  `unknown` with no recovery. Same for `clipboard-get --out`, `trace export`, `session gc/start/end` failures. This
  contradicts CONCEPTS.md:203 ("a caller never sees an unknown disposition for a request that sent no input").
  Side effect: `screenshot out.png --headed` hides the overlay (captured without it) while inline `screenshot --headed`
  captures it. CONFIRMED (traced).
- Test sketch (fails on main): in `src/dispatch/disposition_tests.rs`, `failure_of(AdapterError::timeout("x"),
  &["screenshot", "--app", "Finder", "/tmp/out.png"])` (the `FailingAdapter` fails `list_apps`/`list_windows`);
  assert `disposition.delivery() == NotDelivered`. Main returns `DeliverySemantics::unknown()`. Control:
  the same without the path already passes.
- Hidden cost: overlay Hide/Show round trips on headed `screenshot PATH` (2 acked controls), estimate a few ms.
- Impact: B.
- Proposed explicit form: `enum CommandEffect { ReadOnly, LocalWrite, AppInput }` returned by `Commands::effect()`
  in `src/cli/mod.rs` (one exhaustive match replacing `is_mutating`); settle uses `effect != AppInput`; overlay and
  agent-id checks use `effect == AppInput`; scope success disposition uses `AppInput`. Files: `src/cli/mod.rs`,
  `src/dispatch/mod.rs`, `src/dispatch/disposition.rs`, `src/command_policy/mod.rs`, tests. Size S. Pure behavior fix.
- Overlap: #6 (dispatch preflight TIMEOUT keeps unknown, fixed on branch) is the ref-action sibling. BINARY-1 reuses
  the enum.
- Prior art: none found. Searches: "screenshot disposition unknown", "settle_unknown_delivery", "read-only disposition
  not_delivered"; `git log upstream/main -S"fn settle(" -- src` (introduced in 4c598cde, unchanged since).

### BINARY-6: `settle` infers "sent nothing" from the `INVALID_ARGS` code
- Pieces: `src/dispatch/disposition.rs:17-24` (`sent_nothing = read_only || code == InvalidArgs`).
- States: disposition `Unknown -> NotDelivered` is decided by error code, not by phase (before/after the first input).
- Reachable risk: an app-input command that performs one side effect, then fails with an `INVALID_ARGS` error that
  carries no disposition, is stamped `not_delivered / retry: safe`. Candidate: `press --headed --app X <combo>` focuses
  X's window first (`crates/core/src/commands/press.rs:37-47`), then the macOS adapter can refuse the combo with
  `INVALID_ARGS` "No unmodified key types '…' in the active keyboard layout" (`crates/macos/src/input/keyboard/keyboard_map.rs:113-121`,
  no disposition). The keystroke was not sent (that part is right) but the focus change was. Other post-dispatch
  `INVALID_ARGS` producers (trace budget `trace_write_budget.rs:62`, batch output limit) already set explicit
  dispositions, except when the original result was a non-adapter error (`context/mod.rs:326` gives `unknown`, then
  settle stamps `not_delivered`). SUSPECTED (needs a probe of whether any adapter emits `INVALID_ARGS` after a
  partial delivery; the ACTION agent owns those paths).
- Test sketch: stub adapter whose `press_key` returns `AdapterError::new(InvalidArgs, ..)` after `focus_window` was
  called (record it); assert the error disposition is not `not_delivered` or carries `details.focus_changed`. Main:
  `not_delivered/safe`.
- Impact: B (narrow).
- Proposed explicit form: adapters/commands mark refusals with a phase (`Refusal::BeforeInput`) and settle never
  infers from the code. Size M (touches every `INVALID_ARGS` producer). Alternative S: require every app-input
  `INVALID_ARGS` to carry an explicit disposition and assert it in a contract test.
- Overlap: register ref-action SM4 (disposition lattice).
- Prior art: none found. Searches: "INVALID_ARGS not_delivered delivered", "settle_unknown_delivery".

### BINARY-7: `pre_dispatch_error` collapses non-adapter errors to `INTERNAL`
- Pieces: `src/main.rs:308-318` (`other => AdapterError::new(ErrorCode::Internal, other.to_string())`), used at
  `:157, 160, 163, 177, 185, 187, 197, 260, 272, 296, 299`; `src/batch/preparation.rs:92-95` (same collapse);
  in contrast `src/dispatch/disposition.rs:26-37` (`adapter_form`) keeps `PERM_DENIED` for `Io(PermissionDenied)`,
  and `crates/core/src/response/app_error.rs:18-31` documents that the two codes must not be collapsed.
- States: error code depends on the phase that raised it, not on the error: `Io(PermissionDenied)` raised in
  `CommandContext::new` / lease acquisition (pre-dispatch) -> `INTERNAL`; the same error raised inside a read-only
  command -> `PERM_DENIED`.
- Reachable: `--session S <cmd>` when the state root fails the private-file check; `acquire_liveness_lease` opens
  private lock files and `storage/private_file.rs:321` returns `io::ErrorKind::PermissionDenied`. Mapping CONFIRMED;
  which real producer reaches `main.rs:187` with that io kind is SUSPECTED (PR #411 shows the same class in
  `session/tree.rs`).
- Test sketch (fails on main): in `src/tests/main_tests.rs`, `pre_dispatch_error(AppError::Io(std::io::Error::from(
  std::io::ErrorKind::PermissionDenied))).code() == "PERM_DENIED"`. Main returns `INTERNAL`.
- Impact: B (low).
- Proposed explicit form: route both through one `fn into_adapter_error(AppError) -> AdapterError` (today's
  `adapter_form`) and then stamp the disposition. Files: `src/main.rs`, `src/batch/preparation.rs`,
  `src/dispatch/disposition.rs`. Size S.
- Overlap: PR #411 (open) fixes the producer in `session/tree.rs`; main.rs would still collapse other producers.
- Prior art: PR #411 (vercel-labs, open, "report a denied private state root as PERM_DENIED") is related, not a fix for
  this site. Searches: "pre_dispatch_error", "PERM_DENIED INTERNAL io"; `git log upstream/main -S pre_dispatch_error`.

### BINARY-8: Permission report on a fixed 5 s deadline outside every command and batch budget
- Pieces: `src/main.rs:265-276` (`deadline.map_or_else(Deadline::standard, Ok)`; `deadline` is always `None`),
  `crates/core/src/wait/deadline.rs:8, 53-55` (`DEFAULT_OPERATION_TIMEOUT_MS = 5_000`),
  `src/command_policy/mod.rs:134-140` (`Batch` always requires the report), `src/batch/execution.rs:21` (the batch
  deadline is created only after the report, context and lease work). Dead parameters: `execute_with_deadline(...,
  deadline, baseline)` is only called with `None, None` (`src/main.rs:147`), so `:156-158`, `:198-201`, `:298-300`
  never fire.
- States: per command `PermissionProbe(5 s fixed) -> Preflight -> CommandDeadline(own --timeout)`. The probe's budget
  is not derived from the command's `--timeout`/`--timeout-ms` or the batch's `--timeout-ms`.
- Reachable: Linux with a silent AT-SPI bus: `permissions::report` (`crates/linux/src/system/permissions/mod.rs:12-28`)
  spends up to the full 5 s in `bus.get` + `probe_registry` (plus a 1 s capped X11 capture probe) before
  `snapshot --timeout-ms 500` or `batch --timeout-ms 1000` starts its own clock. That breaks the Linux budget in
  AGENTS.md ("returns its documented code within its timeout plus 1 s"). SUSPECTED (needs a probe on a stopped bus;
  the bus connection may fail fast on some setups).
- Hidden cost: every permission-needing command pays one registry probe + one capture probe + `stale_release::sweep`
  on Linux; macOS pays `AXIsProcessTrusted` + screen-capture preflight (estimate <1 ms). Linux estimate a few ms on a
  healthy bus, up to 5 000 ms on a silent one. Not measured.
- Impact: C.
- Proposed explicit form: compute the command's deadline once in main from its own timeout (`Commands::timeout()`),
  pass it as the inherited deadline (removing the dead `Option` params) and give the probe `deadline.capped(probe
  slice)`. For batch, create the batch deadline before the report. Files: `src/main.rs`, `src/cli/mod.rs`. Size S/M.
- Overlap: Linux perf budget; register CLI SM3.
- Prior art: none found. Searches: "permission report deadline timeout", "silent bus timeout batch"; nearest is
  PR #388 (bound the automation permission prompt to 5 s), unrelated site.

### BINARY-9: Visual debug artifact is create-once, and `mcp` forwards per-command globals to every tool (upstream #11)
- Pieces: `src/visual_debug/mod.rs:56-65` (`create_new(true)`, 0600, opened in `prepare` before dispatch),
  `:76-152` (`finish` writes after dispatch), `src/visual_debug/options.rs:202-229` (`validate`: only snapshot/click),
  `src/main.rs:159` (validate runs for every command, before the `Version`/`Skills` arms), `:294-304`;
  `src/mcp/mod.rs:243-259` (`forwarded_globals` forwards every global given on the command line), `src/mcp/call.rs:128-134`.
  `src/visual_debug/capture.rs:41-64` (`click_before` strict-resolves the ref and screenshots before the click).
- States: artifact path `Absent -> Empty (created in prepare) -> Written (finish)`; a second use of the path is
  refused. Empty is terminal if the process dies or returns between prepare and finish (`main.rs:298-300` is dead today,
  so in practice only a crash leaves it).
- Illegal transitions reachable:
  1. #11 confirmed and wider: `agent-desktop mcp --debug --screenshot x.html` makes every non-snapshot/click tool fail
     with `INVALID_ARGS "--debug supports snapshot and click only"` (including `desktop_skills_get`, which the server
     instructions tell the client to read first), and every snapshot/click after the first fail with "Cannot create
     debug output". CONFIRMED.
  2. Same class: `mcp --wait-for <sel>` forwards `--wait-for` to every child, so every tool without post-action-wait
     fails `validate_wait_for_command` (`src/main.rs:238-250`). CONFIRMED.
  3. Observer effect: `click --debug` strict-resolves the ref in `click_before` before the click's own auto-wait, so a
     ref that the click would wait for (not yet actionable/visible) fails `STALE_REF`/`not_delivered` only in debug
     mode. CONFIRMED by reading; the user-visible frequency is SUSPECTED.
- Test sketch (fails on main): MCP process test (`src/tests/mcp_process.rs` pattern): start `mcp --debug --screenshot
  <tmp>/a.html`, call `desktop_version`; assert `isError == false`. Main: `isError: true`. Second: call
  `desktop_snapshot` twice; assert the second is not "Cannot create debug output".
- Hidden cost: `click --debug` resolves the target twice and captures two frames outside the command deadline (each
  `Deadline::standard()`, up to 5 s). Opt-in only.
- Impact: B.
- Proposed explicit form: `enum DebugTarget { Path(PathBuf) /* CLI, create-once */, Dir { dir, seq } /* MCP */ }`;
  `forwarded_globals` refuses (or rewrites per call) the globals that are per-command (`--debug`, `--screenshot`,
  `--wait-for`, `--wait-for-gone`, `--wait-timeout`) at `mcp` startup with a clear error. Files: `src/mcp/mod.rs`,
  `src/visual_debug/options.rs`, `src/visual_debug/mod.rs`. Size S.
- Overlap: upstream #11; register CLI SM6.
- Prior art: none found beyond the register. Searches: "mcp debug screenshot", "visual debug", "mcp wait-for global";
  `git log upstream/main -S forwarded_globals` (only 6660fbac, the MCP feature). PR #587 (open, MCP read-only mode)
  touches `src/mcp/` but not globals forwarding (title only checked).

### BINARY-10: `session start --cursor` persists in two steps (upstream #12)
- Pieces: `src/dispatch/session.rs:159-169` (`session::execute(Start)` writes the manifest, then
  `show_default_cursor(...)?`), `:185-221` (`cursor_overlay::execute(Enable)` rewrites the manifest, then adapter
  Enable).
- States: `Absent -> Started(overlay disabled) -> Started(overlay enabled) -> Rendered | NotRendered`. A failure of the
  second persist returns `Err` and drops `value` (the new `session_id`), leaving an orphan active session the caller
  cannot name (it shows in `session list`, unnamed unless `--name`).
- Reachable: `cursor_overlay::execute` failing on the manifest rewrite (refstore lock timeout, I/O). Adapter failure
  does not error (it becomes `rendered: false`). CONFIRMED (traced); reach needs an I/O or lock failure.
- Test sketch: hard to inject without a seam; with a seam, make `cursor_overlay::execute` fail and assert the error's
  `details.session_id` is set.
- Impact: B (low).
- Proposed explicit form: attach `details.session_id` (and `partial: "session_started"`) to any error after the
  manifest exists, or write the overlay config in the initial manifest (`StartSessionOptions { cursor_overlay }`), which
  removes the intermediate state. Files: `src/dispatch/session.rs`, core `session/mod.rs`. Size S.
- Overlap: upstream #12; #1 (cursor-overlay write racing session end) touches `set_cursor_overlay`.
- Prior art: none found. Searches: "session start cursor", "session start --cursor fails".

### BINARY-11: Dispatch overlay Hide/Show bracket (upstream #13) and its nested batch form
- Pieces: `src/dispatch/mod.rs:52-57` (`overlay_session` iff mutating && headed && overlay enabled), `:67-69` Hide,
  `:138-140` Show (both results ignored); `crates/core/src/context/options.rs:15-23` (`for_batch` keeps headed and the
  overlay config, so entries bracket again).
- States: renderer `Visible | Hidden`, driven by unpaired broadcasts. Cross-process: #13 as registered. In-process,
  deterministic: a headed batch sends Hide(batch), then each headed mutating entry sends Hide/Show, so the overlay is
  `Visible` from the end of the first mutating entry until the batch's own final Show. Read-only entries in a headed
  batch therefore run with the overlay visible, which is also what a standalone read-only command does, so no wrong
  effect follows. CONFIRMED (traced), impact D.
- Hidden cost: 2 acknowledged controls per headed mutating command, plus 2 per headed batch. Estimate a few ms each
  with a live renderer; not measured.
- Impact: D (C for cost).
- Proposed explicit form: a per-process `OverlaySuppression` guard with a depth count (Hide on 0->1, Show on 1->0)
  owned by the dispatch bracket; cross-process counting belongs to the renderer (#13). Files: `src/dispatch/mod.rs`
  (+1 file for the guard). Size S.
- Overlap: upstream #13; register trace S7.
- Prior art: #178 (closed) / PR #183 (merged, "restore cursor overlay lifecycle for keyboard and clipboard actions")
  established the bracket; nothing on nesting. Searches: "overlay hide show headed", "cursor overlay batch".

### BINARY-12: MCP cancel kills the CLI mid-input on macOS and Windows with no release
- Pieces: `src/mcp/calls.rs:50-56` (`cancel` hands the child to `terminate`), `:68-95` (`terminate`: SIGTERM, 3 s grace
  `TERMINATE_GRACE`, 20 ms `EXIT_POLL`, then kill; non-unix kills at once); the doc comment claims SIGTERM "lets the CLI
  release held keys and buttons first", which is true only on Linux (`crates/linux/src/input/x11/signal_release.rs`,
  enabled by `src/main.rs:365`). macOS release lives in `Drop` guards (`crates/macos/src/input/mouse/mouse_drag.rs:162-169`
  `DragReleaseGuard`, `crates/macos/src/input/keyboard/keyboard_event.rs:109-115` `KeyReleaseGuard`), which do not run
  when SIGTERM's default action terminates the process; no macOS SIGTERM handler exists (grep: only
  `system/process/mod.rs` sends SIGTERM).
- States: child input `Idle -> ButtonDown/KeyDown -> Released`. Cancel moves `ButtonDown -> ProcessGone` with no
  `Released`.
- Illegal transition: an MCP client cancels a `desktop_drag` (drag `--duration`/`--drop-delay` keep the button down for
  as long as asked) or a long `desktop_type`/`desktop_press` mid-keystroke; on macOS the HID-posted left button (or
  key) stays logically down after the CLI dies, so the next pointer move from the user or the agent becomes a drag or
  drop on whatever is under it. Windows: `TerminateProcess` mid-`SendInput` sequence, same outcome. SUSPECTED (needs a
  probe: does WindowServer / Windows input stack auto-release injected button state when the poster dies?).
- Test sketch (probe, macOS): run `agent-desktop mcp`, call `desktop_drag` with `duration: 5000`, send
  `notifications/cancelled` after 500 ms, then read `CGEventSourceButtonState(kCGEventSourceStateHIDSystemState,
  kCGMouseButtonLeft)`; expect `false`. Unit level: a test that `terminate` is only used on platforms with a release
  handler, or that the CLI installs one (assert a SIGTERM handler is registered after `run()` on macOS).
- Hidden cost: none (3 s worst-case grace per cancel on Linux).
- Impact: A (wrong action delivered after cancel), if the probe confirms.
- Proposed explicit form: make release-on-signal a `PlatformAdapter`-level capability the binary enables on every OS
  (macOS: a signal-pipe watcher like Linux's that triggers the active release guard; Windows: start children with
  `CREATE_NEW_PROCESS_GROUP` and send `CTRL_BREAK_EVENT`, handled by a console ctrl handler that runs the guards), and
  have `terminate` wait for the release before killing. Files: `src/mcp/calls.rs`, `src/main.rs`, macOS and Windows
  input crates. Size M.
- Overlap: #5 (same `cancel`/`terminate` path; the fix branch keeps `terminate` unchanged).
- Prior art: none found. Searches: "mcp cancel" (finds #599, PR #602, merged PR #525 "honor mcp cancellation"),
  "SIGTERM release held keys", "notifications/cancelled", "mcp drag cancel mouse button";
  `git log upstream/main -S request_exit -- src` (only 6660fbac).

### BINARY-13: MCP call registry, pool and EOF lifecycle (beyond #5)
- Pieces: `src/mcp/mod.rs:43-95` (loop, `running` JoinHandles, `MAX_CONCURRENT_CALLS = 8`, busy check at :76-80
  before the duplicate-id check at :81-88), `:128-149` (`respond`: `finish` gates the write), `src/mcp/call.rs:135-157`
  (spawn under lock, read both pipes to EOF, `take`, `wait`), `src/mcp/calls.rs:17-66`.
- States: per id `Absent -> Admitted -> Running(child) -> Taken -> Absent`; server `Reading -> Draining -> Exited`.
- Checked and found sound on main (other than #5): pipes are read to EOF before `take`, and every long-lived
  grandchild the CLI spawns nulls its stdio (overlay renderer `crates/macos/src/system/cursor_overlay/spawn.rs:113-121`,
  Windows renderer with no inherited handles `crates/windows/src/system/cursor_overlay/spawn.rs:228-265`, Linux launch
  `crates/linux/src/system/launch/launch_spawn.rs:73-76`, Linux clipboard owner `owner_spawn_spec.rs:59-61`), so a call
  cannot hang on an inherited pipe; one response per admitted id (the write is gated by `finish`); a cancelled call
  writes nothing, so partial output is never returned.
- Minor (D): a duplicate in-flight id gets a busy *result* when the pool is full and a `-32600` error otherwise
  (ordering at :76-88), both carrying the running call's id (a client protocol violation, but the reply differs by pool
  state); detached `terminate` threads are not counted in the pool and not joined at stdin EOF, so more than 8
  children can be alive and the server can exit mid-grace (child gets SIGTERM but no follow-up kill); there is no
  server-side wall-clock bound, so a hung child holds a slot until the client cancels.
- Impact: D.
- Proposed explicit form: the fix branch's `Slot` enum plus `Slot::Terminating(JoinHandle)` counted by the pool and
  joined at EOF; check duplicate id before pool capacity. Size S.
- Overlap: #5 / PR #602.

### BINARY-14: Batch item contexts frozen at prepare time
- Pieces: `src/batch/preparation.rs:42-56` (every item's context built, and `preflight_context` run, before entry 0);
  `crates/core/src/context/mod.rs:238-275` (`for_batch_item`: trace sink, artifacts mode, liveness lease, cursor
  overlay incl. `multi_agent` read once); `src/batch/execution.rs:97-101` (only deadline and baseline are refreshed).
- States: per item `Prepared(snapshot of manifest at t0) -> Executed(at t0 + up to 60 s)`.
- Reachable divergence: only cross-process. In-process, the batch cannot change another item's manifest state: new
  session ids are random (`crates/core/src/session/mod.rs:308`), `cursor-overlay` is refused in batch
  (`src/batch/mod.rs:103-106`), and `session end` is caught by BINARY-3's gate. A concurrent `cursor-overlay enable
  --multi-agent` on the same session mid-batch lets later entries act without `--agent-id`, which a standalone command
  started then would refuse. SUSPECTED, low. Side note: batch allows `session start` with `cursor: true`
  (`src/batch/mod.rs:268-279`) while refusing `cursor-overlay`, which spawns a renderer for a session no later entry can
  name (D).
- Hidden cost: up to 64 liveness leases and manifest reads acquired up front and held for the whole batch.
- Impact: D.
- Proposed explicit form: keep prepare for parse/arg validation only; build the item context and run
  `preflight_context` at dispatch time (`PreparedCommand { command, session }`, context built lazily). Size S.
- Overlap: register sessions SM1/SM2 (leases).

---

Files read: `src/main.rs`, `src/dispatch/{mod,session,cursor_overlay,disposition,overlay_skipped,system,observation}.rs`,
`src/batch/{mod,execution,preparation,result_entry,bounded_json}.rs` + tests, `src/command_policy/mod.rs`,
`src/mcp/{mod,call,calls,catalog}.rs`, `src/visual_debug/{mod,options,capture}.rs`, `src/cli/mod.rs`,
`src/cli_args/{stdin,batch}.rs`, `src/diagnostic/mod.rs`; core `context/*`, `wait/deadline.rs`, `trace/mod.rs`,
`trace/trace_write_budget.rs`, `response/*`, `session/mod.rs`, `commands/{wait,wait_event_filter,launch,press,screenshot}.rs`;
platform spawn sites listed in BINARY-13. `cli_args/` and `diagnostic/` hold no lifecycle state (static clap tables and
token parsing): no entries.
