# State machines

This file maps the explicit and implicit state machines in agent-desktop: their states, invariants, and
transition tables. It also keeps a register of invalid states and transitions the code can reach,
including through retries, crashes between steps, and concurrent processes or threads.

Concurrency here mostly means several CLI processes sharing state under the state root (sessions, refs,
locks, traces). Inside one process it means FFI host threads, MCP call threads, and cursor-overlay helpers.
Line references are approximate; follow the named function when a line has moved.

## Findings register

Ordered by impact. "Fixed" findings have a regression test that fails without the fix.

| # | Finding | Impact | Status |
|---|---|---|---|
| 1 | A `cursor-overlay` write racing `session end` writes `ended_at: null` back and reopens the session | Medium | Fixed ([#601](https://github.com/vercel-labs/agent-desktop/issues/601)) |
| 2 | A command reads the manifest before its liveness lease, so a session GC reaps in between is recreated without a manifest and keeps receiving trace | Medium | Fixed ([#601](https://github.com/vercel-labs/agent-desktop/issues/601)) |
| 3 | Ending a full-artifacts session deletes refs written by `snapshot --root`, because the stale trace copy counts as a duplicate | Medium | Fixed ([#597](https://github.com/vercel-labs/agent-desktop/issues/597)) |
| 4 | A process killed while rewriting `.trace-usage.json` leaves it empty, and the session never traces again | Medium | Fixed ([#600](https://github.com/vercel-labs/agent-desktop/issues/600)); crash-only, no deterministic test |
| 5 | MCP cancel frees the id while the call runs; a reused id gets the cancelled call's output and an uncancellable child | Medium | Fixed ([#599](https://github.com/vercel-labs/agent-desktop/issues/599)) |
| 6 | A retryable live-read `TIMEOUT` in the dispatch preflight keeps `delivery: unknown`, so auto-wait stops early with `retry: unknown` | Medium | Fixed ([#598](https://github.com/vercel-labs/agent-desktop/issues/598)) |
| 7 | Unscoped `wait --event app-terminated` confirms only the first match, so an app that closed to tray hides a real exit | Medium | Fixed ([#598](https://github.com/vercel-labs/agent-desktop/issues/598)) |
| 8 | `end_session` discards ref scaffolding without `refstore.lock` and can delete a pointer another process just wrote | Low | Fixed with #1 (`end_session` now holds the lock) |
| 9 | FFI: a failing call returns `AD_RESULT_OK` when the thread's last-error TLS is already destroyed (`ffi/src/error.rs`) | Low to medium | Open |
| 10 | A complete snapshot whose ref allocation passes the inherited deadline fails with `TIMEOUT`, while an incomplete one is saved (`snapshot/mod.rs` `save_refmap`) | Low to medium | Open; needs a policy decision |
| 11 | `mcp --debug --screenshot` fails every snapshot or click after the first, because the artifact file is create-once | Low | Open |
| 12 | `session start --cursor` that fails after creating the session drops the session id from the error | Low | Open |
| 13 | Cursor overlay Hide/Show is a switch, not a count, so concurrent headed commands re-show it mid-action | Low | Open; intent unconfirmed |
| 14 | `ad_adapter_destroy` drops the adapter while holding the global adapter lock | Low | Open |
| 15 | Manifest-less session directories GC can never reclaim: crash inside `start_session`, a lock-open race with GC, or a partial GC delete that also aborts the GC loop | Low | Open |
| 16 | Trace reader pairs commands by name in one slot, so overlapping same-named FFI commands are mispaired | Low | Open |
| 17 | With `--wait-for`, the post-action screenshot runs after the wait against the spent action deadline | Low | Open |
| 18 | The pre-action screenshot is written but not recorded when the interaction lease fails | Low | Open |
| 19 | The notification wait's starting counts never go down, so a dismissed-then-identical notification is missed | Low | Open (likely) |
| 20 | `include/agent_desktop.h` says the log callback is per thread; it is process-wide and can fire after unregister. `threading.md` says destroy revokes native handles; it does not | Low | Open (docs) |

## Sessions and leases

Paths are relative to `crates/core/src/`.

### 1. Session manifest lifecycle (`session/mod.rs`, `session/manifest.rs`)

States (on-disk under `sessions/<id>/`):
- **Absent**: no directory.
- **Bare**: directory without a readable `session.json`. Created by any writer under `--session <id>` (refs, trace, lock files). Legal by design (CONCEPTS.md:157). Invisible to `list_sessions`/`gc` (`session/mod.rs:291`).
- **Active**: manifest with `ended_at = None`.
- **Ended**: manifest with `ended_at = Some(t)`. This should be terminal: `trace_enabled()`/`artifacts_full()` go false (`manifest.rs:34-40`), and commands are refused (`context/session_scope.rs:17-31`).
- **Removed**: GC deleted the directory, which returns it to Absent.

Invariants:
- I1: Ended is terminal. Nothing writes `ended_at = None` over an Ended manifest. Every manifest read-modify-write holds `refstore.lock` (`liveness::lock_started_session`), so the ended check and the write are atomic.
- I2: A session with a held liveness lease or refstore lock is never removed.
- I3: A Bare id never gets trace segments (CONCEPTS.md:159).

| From | Event | To | Where |
|---|---|---|---|
| Absent | `start_session`: `tree::create` | Bare (dir + `trace/`) | session/mod.rs:314, tree.rs:9-29 |
| Bare | `start_session`: `write_manifest` | Active | session/mod.rs:324 |
| Active | `set_cursor_overlay` (read, modify, write under `refstore.lock`) | Active | session/mod.rs:117-138 |
| Ended | `set_cursor_overlay` | refused | session/mod.rs:129 |
| Active | `end_session` (under `refstore.lock`) | Ended (+ discard byte-identical ref scaffolding if Full) | session/mod.rs:330-350 |
| Ended | `end_session` | Ended (no-op) | session/mod.rs:338 |
| Ended / Active (older_than, idle) | `gc`: under the refstore lock, no lease, no activity | Removed | session/gc.rs:56-92 |
| Absent | any writer (trace, refs, lock open with create) | Bare | trace/trace_file.rs:44-53, storage/private_file_parent.rs:44-69 |

### 2. Session liveness lease (`session/liveness.rs`)

States per lease file `liveness/<pid>-<ns>-<seq>.lock`: **NoFile**, then **FileLocked** (held), then **FileUnlocked** (released or crashed holder; the file stays). `any_held` uses `FileLock::is_held`, which tries the lock on a fresh open file description.

Invariant: a lease is created only while the refstore lock is held and the manifest exists (liveness.rs:39-44). GC checks `any_held` under the same lock (gc.rs:81).

| From | Event | To | Where |
|---|---|---|---|
| NoFile | `acquire`: dir exists, refstore lock taken, manifest present | FileLocked | liveness.rs:38-55 |
| (none) | `acquire`: dir missing or manifest missing | `Ok(None)` (lease-less command) | liveness.rs:39-45 |
| FileLocked | last `Arc` clone dropped | lock closed, then file unlinked (NoFile) | liveness.rs:26-31 |
| FileLocked | process killed | FileUnlocked (counts as not held) | file_lock.rs |

The lease is taken by `CommandContext::new` and `for_batch_item` before either reads the manifest (context/mod.rs:92, 249), and for the FFI adapter's lifetime (`crates/ffi/src/adapter.rs:22`). A command that loses the race with GC gets no lease, reads no manifest, and records no trace.

### 3. Process lease guard (`session/process_lease_guard.rs`)

States: global `PROCESS_LEASE_HELD` is **Free** or **Held**.

| From | Event | To | Where |
|---|---|---|---|
| Free | CAS succeeds, deadline not expired | Held | process_lease_guard.rs:16-24 |
| Free | CAS succeeds, deadline expired | Free + Timeout | process_lease_guard.rs:20-22 |
| Held | CAS fails | spin 1 ms until the deadline, then Timeout | process_lease_guard.rs:26-31 |
| Held | guard dropped (the last `Arc` in `InteractionLease`) | Free | process_lease_guard.rs:40-44 |

Invariant: at most one interaction lease per process. A thread that already holds the lease and asks for it again waits until its deadline runs out.

### 4. Interaction lease (`session/interaction_lease.rs`)

States: **None**, **Guarded** (platform guard, no file), **FileHeld** (process guard + flock on `/tmp/agent-desktop-<uid>/interaction.lock`), **Adopted** (process guard + flock through an inherited fd that shares the parent's open file description).

| From | Event | To | Where |
|---|---|---|---|
| None | `acquire_unix_interaction_lease` | FileHeld (the process guard drops if the flock times out) | interaction_lease.rs:108-119 |
| None | `adopt_inherited_*` (dev/ino match the canonical path) | Adopted | interaction_lease.rs:153-174, file_lock.rs:54-89 |
| None | `guarded` / `guarded_with_contention` | Guarded | interaction_lease.rs:23-36, 90-104 |
| any | `with_deadline` | same ownership (`Arc` clones), new deadline | interaction_lease.rs:43-53 |
| FileHeld / Adopted | last clone dropped | None (close, not unlock) | file_lock.rs:7-10 |

I found no reachable invalid transition in 3 or 4 inside this scope.

### 5. Session affinity and session tree

`SessionAffinity` (session_affinity.rs) is a plain `Option<String>` with no transitions. `tree::create` is idempotent `mkdir -p` (mode 0700) of the session dir and its `trace/`, after checking that the root is private (tree.rs:9-29).

## Refs and storage

Paths are relative to `crates/core/src/`.

### SM1: Snapshot directory lifecycle (`<base>/snapshots/<id>/refmap.json`)
States: Absent → DirOnly (parent created, only `.refmap.json.<nonce>.tmp` present) → Committed (refmap.json) → Rewritten (drill-down replaced refmap.json in place) → Removed (pruned / discarded / session gc).
Invariants:
- I1. Every mutation of `snapshots/` and `latest_snapshot_id` happens under `refstore.lock` (`with_write_lock`, refs_store.rs:247-250).
- I2. `latest_snapshot_id` names a Committed directory (prune skips latest, refs_store_prune.rs:114; discard drops the pointer when its dir is gone, refs_store_prune.rs:64-69).
- I3. Ids are never reused (random base36, refs/mod.rs:216-224). The contents are NOT immutable: `update_existing_snapshot` rewrites them.
- I4. A snapshot dir is discarded only when the trace holds a byte-identical copy of *that* refmap (refs_store_prune.rs:39-64).

| From | Event | To | Where |
|---|---|---|---|
| Absent | `save_new_snapshot*` → `save_snapshot_unlocked` → `write_atomic` ensures parent | DirOnly | refs_store.rs:58-60, 186-191; storage/private_file.rs:124 |
| DirOnly | rename tmp → refmap.json | Committed | storage/private_file.rs:132 |
| DirOnly | crash before rename | DirOnly (orphan tmp, swept after 60 s only if dir is latest, or during eviction) | refs_store_prune.rs:21-37, 95-97 |
| Committed | `set_latest_unlocked` | Committed + latest | refs_store.rs:61, 193-196 |
| Committed | drill: `update_existing_snapshot` (root entry unchanged) | Rewritten | refs_store.rs:70-87; snapshot/snapshot_ref.rs:110-120 |
| Committed/Rewritten | root entry changed or missing | unchanged, `STALE_REF` | refs_store.rs:80-82 |
| Committed | >128 dirs on save, oldest by dir mtime, not latest | Removed (down to 96) | refs_store_prune.rs:72-120 |
| Committed | `end_session` (Full artifacts) and `trace/refmaps/<id>.json` is byte-identical to `refmap.json` | Removed (under `refstore.lock`) | session/mod.rs:342-348; refs_store_prune.rs:46-72 |
| (no pointer, legacy store) | `load_latest` → `migrate_legacy_latest` | Committed + latest | refs_store.rs:104-114, 263-283 |

### SM2: File lock (`FileLock` / `RefStoreLock`)
States: Unopened → Opened → Contended(n) → Held → Released (drop/close, never unlock); terminals TimedOut and Error.
Invariants: released by close only (file_lock.rs:7-10); the budget is min(2 s, inherited deadline) unless detached (refs_lock.rs:12-23; wait/deadline.rs:23-30).

| From | Event | To | Where |
|---|---|---|---|
| Unopened | `open_private_lock(create)` | Opened | file_lock.rs:18-20 |
| Opened | deadline already expired (checked before the first try_lock) | TimedOut | file_lock.rs:102-104 |
| Opened | try_lock Ok, deadline not expired | Held | file_lock.rs:105-115 |
| Opened | try_lock Ok, deadline expired | TimedOut (lock dropped) | file_lock.rs:107-109 |
| Opened | WouldBlock | Contended(n+1), sleep ≤10 ms | file_lock.rs:116-123 |
| Contended | remaining == 0 | TimedOut | file_lock.rs:119-121 |
| Held | guard dropped | Released | file_lock.rs:11-15 |
| — | `is_held` probe: try_lock+unlock on a fresh description | no change | file_lock.rs:23-35 |

### SM3: Atomic private write (`write_atomic_with`)
States: Start → ParentEnsured → DestValidated → TmpCreated → Written+fsynced → Renamed → Validated → ParentSynced (Ok). An error before rename removes tmp. An error after rename returns Err but the destination is already replaced.

| From | Event | To | Where |
|---|---|---|---|
| Start | `ensure_parent` | ParentEnsured | storage/private_file.rs:124 |
| ParentEnsured | `validate_destination` | DestValidated | :125 |
| DestValidated | `create_new` tmp (≤32 tries) | TmpCreated | :126, 203-228 |
| TmpCreated | write_all + sync_all | Written | :127 |
| Written | rename | Renamed | :132, 257-259 |
| Renamed | validate + dir fsync | Ok | :133-134 |
| any | Err | tmp removed (no-op after rename) | :136-138 |

### SM4: Resolve attempt (`ResolveAttemptOutcome`)
States: DeadlinePassed | Resolved(handle) | Failed(terminal) | Failed(retryable).

| From | Event | To | Where |
|---|---|---|---|
| start | outer deadline has no slice left | DeadlinePassed | refs/ref_resolve_deadline.rs:47-49 |
| start | attempt Ok, outer still live | Resolved | :52 |
| start | attempt Ok, outer expired | DeadlinePassed (handle dropped) | :51 |
| start | non-Timeout error, or Timeout with retryable=false/Unsafe | Failed(terminal) | :58-65 |
| start | Timeout, outer expired | DeadlinePassed | :66-68 |
| start | Timeout, outer live | Failed(retryable) | :69 |
`land_saved_ref` falls back to a full `resolve_saved_ref` when landing or confirmation fails (refs/resolve_saved_ref.rs:58-77). No invalid transitions found here.

### SM5: Snapshot ↔ trace refmap copy pairing (Full artifacts)
States per id: NoCopy | Copy(matches refmap.json) | Copy(stale).

| From | Event | To | Where |
|---|---|---|---|
| NoCopy | snapshot/find/wait saves, then `copy_refmap_if_full` | Copy(matches) | snapshot/mod.rs:182-183; trace/trace_artifact_budget.rs:99-118 |
| NoCopy | budget exhausted or write failed | NoCopy | trace/trace_artifact_budget.rs:109-117 |
| Copy(matches) | drill rewrites refmap.json; copy is "if absent", so it is skipped | **Copy(stale)** | snapshot/snapshot_ref.rs:110-126; trace/trace_artifact_budget.rs:105-107 |
| Copy(matches) | `end_session` discard | snapshot dir Removed | refs_store_prune.rs:60-64 |
| Copy(stale) | `end_session` discard | snapshot dir kept (it holds the only copy of the drilled refs) | refs_store_prune.rs:60-64 |

## Trace and cursor overlay

Paths are relative to `crates/core/src/trace/` and `crates/core/src/cursor_overlay/`.

### S1. Trace writer: `TraceWriterState` × `TracePending` (trace_state.rs:9-21, mod.rs:38-152)
- **States:** pending is one of {None, File(path), SegmentDir(dir)} and never changes. The writer is one of {Unopened, Open(file), Failed}.
- **Invariants:**
  - pending=None implies the writer ends Failed.
  - File plus a successful eager open starts the writer Open.
  - Failed is terminal for that `TraceState`.
  - `has_sink` is true unless pending=None or the writer is Failed.
- **Transitions:**

| from | event | to | where |
|---|---|---|---|
| (build) | explicit path opens | Open | mod.rs:39-43 |
| (build) | explicit open fails, non-strict, not INVALID_ARGS | Failed | mod.rs:45-48 |
| (build) | explicit open fails, strict or INVALID_ARGS | build Err | mod.rs:44 |
| (build) | segment dir / none | Unopened | mod.rs:50-53 |
| Unopened | ensure_writer, pending=None | Failed | mod.rs:131-133 |
| Unopened | ensure_writer, open ok | Open | mod.rs:139-142 |
| Unopened | open fails, strict or INVALID_ARGS | Unopened (Err returned, retried next emit) | mod.rs:144-145 |
| Unopened | open fails otherwise | Failed | mod.rs:146-149 |
| Open / Failed | ensure_writer | unchanged | mod.rs:126-127 |

### S2. `meta_written` flag (mod.rs:187-203)
- **States:** false or true. The flag belongs to one `TraceState`. The segment file name is shared process-wide (trace_file.rs:3-16).
- **Invariant:** each file gets exactly one `trace.meta` line, and it is the first line. Both writes happen under the per-state emission mutex and an flock on the file (mod.rs:81-85, 344-358).

| from | event | to | where |
|---|---|---|---|
| false | file len > 0 (another state or process wrote first) | true | mod.rs:196-198 |
| false | meta written | true | mod.rs:200-201 |
| false | meta write fails (budget/size) | false | mod.rs:200 `?` |

### S3. Event `seq` (mod.rs:18, 272-275)
- A process-global `AtomicU64`. It is consumed even when the line is then rejected (mod.rs:287-289), so seq can have gaps.
- The reader does not use seq. It merges on `(ts_ms, writer_pid, position)` (trace_read/merge.rs:19-24). No reachable invalid state matters here.

### S4. Session write budget ledger `.trace-usage.json` (trace_write_budget.rs)
- **States:** the ledger is absent, valid `(bytes, events, entries)`, or unparseable. The `.trace-budget.lock` lock is held or free.
- **Invariants:**
  - The ledger is at least what is on disk.
  - Each value stays at or under the reader's `ReadBudget` limits (trace_read/budget.rs).
  - Every read-modify-write happens under the lock.
  - An unparseable ledger fails closed (test `interrupted_recording_ledger_stops_writes_without_losing_recorded_events`, trace_read/input_tests.rs:219-228).

| from | event | to | where |
|---|---|---|---|
| absent | acquire | valid (scanned via `initial_usage`) | :31, :75-114 |
| valid | acquire, own segment absent | entries+1, persisted | :39-45 |
| valid | reserve_line | bytes+n, events+1, persisted | :51-55 |
| valid | persist over a limit | Err, ledger unchanged | :58-66 |
| valid | persist: atomic temp write and rename (`write_private_file`) | valid (old or new, never partial) | :64-67 |
| unparseable | acquire | Err forever (non-strict drops events, strict fails every command). No longer reachable from a killed writer. | :30 |

### S5. Artifact budget ledger `.artifact-usage.json` (trace_artifact_budget.rs)
- **States:** absent or corrupt (rescanned from `screens/` and `refmaps/`), or valid. Updates are atomic (`write_private_file` → `write_atomic`, refs/mod.rs:270).

| from | event | to | where |
|---|---|---|---|
| any | load, ledger missing or undecodable | rescan + persist | :149-161 |
| valid | screenshot reserve | +bytes, +1 | :86-91 |
| reserved | write fails, file absent | rolled back to the snapshot | :92-94, :182-189 |
| reserved | crash before write | over-counted (conservative) | :91-92 |

### S6. Reader `ReadBudget` and command pairing (trace_read/budget.rs, merge.rs:95-155)
- **ReadBudget:** bytes, events and entries count down, and the first overflow sets `exceeded`. The writer budget mirrors these limits.
- **Command pairing:** `open` is a map keyed by `segment:writer_pid:command`.

| from | event | to | where |
|---|---|---|---|
| key absent | command.start | open[key] = info | merge.rs:118-127 |
| key present | command.start | open[key] **overwritten** | merge.rs:119 |
| key present | command.end | removed | merge.rs:129 |
| key absent | command.end | "Unpaired command.end" warning | merge.rs:129-133 |
| leftover | end of scan | "Unpaired command.start" warning | merge.rs:137-153 |

### S7. Cursor overlay control protocol (cursor_overlay/control.rs, submit.rs, cursor_hold.rs)
- **Renderer states:** absent, visible, hidden, resting. Controls are Enable, Present(phase), Hide, Show, Disable.
- **Phases:** Travel and Drag are pre-dispatch and acknowledged. Effect, Caret and CaretFollow are acknowledged. Hide, Show and Disable are acknowledged (control.rs:265-276).
- **Per-cursor lock:** `cursor/session.lock` or `cursor/agent-<id>.lock` (cursor_hold.rs:24-41). It is held from travel until the effect has played, and taken before the interaction lease.

| from | event | to | where |
|---|---|---|---|
| any | Present Travel or Drag (needs >1000 ms deadline left) | visible at destination | submit.rs:15-24, 55-74, 146-165 |
| any | Present Travel with too little deadline | no-op, returns false | submit.rs:16-20 |
| visible | delivered input | Present Effect | submit.rs:247-266 |
| drag armed | drag not delivered | Hide (`cancel_drag`) | submit.rs:184-195, commands/drag.rs:170-171 |
| any | headed mutating command starts / ends | Hide / Show broadcast | src/dispatch/mod.rs:52-68, 138-140 |
| hidden | any Present | visible (renderers re-front on draw) | crates/macos/.../cursor_overlay_bridge.m:406, crates/windows/.../surface_host.rs:92-98 |
| resting | CaretFollow | stays resting, acknowledged | crates/macos/.../child.rs:89-92 |
| any | Disable | child exits, socket removed | crates/macos/.../child.rs:84-88 |

## Ref actions and waits

Paths are relative to `crates/core/src/`.

### SM1. Ref-action auto-wait outer loop (`ref_action/ref_action_wait.rs::execute_with_deadline`)

States: `Polling` (read-only, no lease) → `PreArtifact` → `CursorHeld` → `LeaseAcquiring` → `Dispatching` (lease held) → `Done` | `Refused` (back to `Polling`) | `Failed`.

Invariants:
- I1: nothing is sent to the app without the lease.
- I2: the loop goes back to the wait only when the refusal is `not_delivered`, the resolution failure is retryable, and the budget remains (`resolves_again`, :140-144).
- I3: every pre artifact that is captured is emitted with its post artifact.

| From | Event | To | Where |
|---|---|---|---|
| Polling | `execute_poll_loop` Ok | PreArtifact | ref_action_wait.rs:84-90 |
| Polling | poll error | Failed (with `details.refusal` if a refusal came earlier) | :85-86 |
| PreArtifact | screenshot written | CursorHeld | :91-97 |
| CursorHeld | `before_lease` travel, then `acquire_interaction_lease` Ok | Dispatching | :98-102 |
| CursorHeld | lease error | Failed (pre artifact never emitted) | :99-102 |
| Dispatching | deadline already expired | Failed (pre artifact never emitted) | :103-106 |
| Dispatching | `execute_single_shot` Ok | Done (lease returned to caller) | :111-119 |
| Dispatching | Err, `polls && resolves_again` | Refused → sleep → Polling | :121-131 |
| Dispatching | Err otherwise | Failed | :124-125 |

### SM2. Read-only poll loop state (`ref_action/ref_action_poll.rs`, `ref_action_poll_state.rs`)

States: `RefActionPollState { last_report, last_settled_identity_error, saw_ambiguity, expected_bounds_hash, live_read_incomplete_only: None|Some(true)|Some(false), counters }`.

Invariants:
- `live_read_incomplete_only` starts at `None`. It moves to `Some(true)` only if every preflight error so far was a live read incomplete. Once it is `Some(false)` it stays there.
- The settled identity error is cleared on any successful resolve.

| From | Event | To | Where |
|---|---|---|---|
| any | resolve `DeadlinePassed` | timeout error (live_read_incomplete > settled identity > generic) | poll.rs:44, 163-188 |
| any | resolve Err, `StaleRef` with `complete: true` | terminal | poll.rs:109-119 |
| any | resolve Err that isn't retryable | terminal | poll.rs:120-123 |
| any | resolve Err that is retryable | `record_resolve_error` (lri := false, settled identity stored if complete) → sleep | poll.rs:124-127, state.rs:292-303 |
| any | resolved | `note_resolved` (settled identity cleared) → preflight | poll.rs:53-59, state.rs:305-307 |
| preflight | Ok, or a focus/scroll recovery is pending | return Ok(state) | poll.rs:60-72 |
| preflight | permanent or non-retryable error | terminal | poll.rs:83-85 |
| preflight | retryable error | `record_preflight_error` (bounds hash adopted, last_report) → sleep 16 or 100 ms | poll.rs:89-100 |

### SM3. Dispatch-time stable preflight sampler (`ref_action/mod.rs::stable_preflight`, `actionability/stability_sampler.rs`)

States: `StabilitySampler { previous: Option<Rect>, consecutive, stable_since }`.

Invariant: the sampler reports stable only after 3 or more matching samples and 34 ms or more of span.

| From | Event | To | Where |
|---|---|---|---|
| any | bounds `None` (or invalid while geometry is pending) | reset (previous None, consecutive 0) | stability_sampler.rs:24-29, mod.rs:286-289 |
| previous ≈ bounds | sample | consecutive+1 | stability_sampler.rs:30-34 |
| previous ≠ bounds | sample | consecutive 1, stable_since := now | :35-38 |
| stable | `observe` true | strict `check_live_with_stability` | mod.rs:290-297 |
| not stable | slice is zero | `Timeout`, explicitly `not_delivered` | mod.rs:298-301 |
| any | `get_element_bounds` Err | error marked `not_delivered` by the caller | mod.rs:94, 282-284 |

### SM4. Delivery-disposition lattice for one ref action (`DeliverySemantics`)

States: `Unknown → NotDelivered | DeliveryUncertain | DeliveredUnverified | DeliveredVerified`.

Invariant: any refusal raised before `execute_verified_action` must be `NotDelivered`. That is what `retry: safe` means, and `resolves_again` depends on it.

| From | Event | To | Where |
|---|---|---|---|
| Unknown | `mark_pre_dispatch_resolution_failure` | NotDelivered | ref_action/mod.rs:188-197 |
| Unknown | `after_scroll` | NotDelivered | ref_action_wait_support.rs:389-392 |
| Unknown | `check_live_with_stability` / `check_live_before_focus` error | NotDelivered | actionability/live.rs:56,64,90,98 |
| Unknown | `check_live_with_stability_or_gap` error in the dispatch preflight | NotDelivered (marked by the caller) | ref_action/mod.rs:94 |
| * | trace failure after dispatch | DeliveredUnverified | ref_action/mod.rs:238-243 |

### SM5. Pre-lease cursor travel (`ref_action/presentation.rs` thread-local `TRAVELLED_BEFORE_LEASE`)

States: `None` | `Some(point)`.

| From | Event | To | Where |
|---|---|---|---|
| any | `before_lease` entry | None | presentation.rs:51 |
| None | pre-lease travel submitted | Some | :60-68 |
| Some | `before_dispatch` destination differs by ≥1.5 px | None + travel under the lease | :23-35 |
| any | `after_dispatch` | None (take) | :155 |

Every caller goes through `execute_with_deadline`, which resets the state on each iteration, so no stale value is carried across actions.

### SM6. Event wait baseline (`commands/wait_event.rs`, `wait/signals.rs`)

States: `baseline: Option<SignalBaseline>`, `seen: Option<SignalBaseline>` (disappearance only, grow-only).

| From | Event | To | Where |
|---|---|---|---|
| baseline None | capture Ok | baseline = seen = current | wait_event.rs:95-98 |
| baseline Some | diff produces a matching event that `confirm_app_terminated` accepts (every match is tried) | Found | :105-121 |
| baseline Some | no confirmed match, disappearance | seen := merge(seen, current) | :122-124, signals.rs:211-223 |
| any | retryable capture error | last_error | :128-130 |
| any | deadline | Timeout | :70-89, 134-143 |

### SM7. Notification wait baseline (`commands/wait_notification.rs`)

States: `baseline: Option<HashMap<Fingerprint, count>>`. The baseline is fixed after the first successful list.

| From | Event | To | Where |
|---|---|---|---|
| None | list Ok | Some(counts) | :46-48 |
| Some | some fingerprint count > baseline count | Matched | :49-58, 118-131 |
| Some | otherwise | unchanged | — |

### SM8. Post-action observation probe (`commands/post_action_observation.rs`)

Outcome is one of `timed_out | expected_state_observed | process_exited | process_crashed | owner_unavailable | observation_failed | identity_unavailable`. `latest` is reset to `timed_out` on every completed poll (:56). The final value is `owner_unavailable` or `timed_out` when nothing terminal happened (:106-110). It only runs on `DeliveryUncertain` (helpers.rs:134-139). I found no invalid transition here.

### SM9. Selector wait (`commands/wait_selector.rs`)

`last_built: Option<SnapshotResult>` and `last_error`. A fresh build runs near the deadline only when `last_built` is None (:111-120). The timeout response persists `last_built`, or a 600 ms diagnostic build (:172-197). I found no invalid state, but the snapshot persisted on timeout can be older than the final poll.

### SM10. Process, permission and window state value types (`system/*.rs`)

These are stateless classifications with no transitions. `ProcessState` drives enrichment: `Unresponsive` plus a matching identity becomes `APP_UNRESPONSIVE` with `retryable: false`, and `Exited` plus `StaleRef` adds a suggestion (ref_action_wait_support.rs:262-350). Enrichment is skipped once the deadline has expired (ref_action_wait.rs:52-58). `launch` (lease → `launch_app` → CDP verify) and `close_app` (resolve → lease → revalidate → close) are linear sequences, and `close_app` revalidates under the lease.

## CLI, batch, and MCP

Paths are relative to `src/` (the binary crate) unless the path says `crates/`.

### SM1. MCP in-flight call registry (`Calls`, keyed by `id.to_string()`)
States per id: **Absent**, **Admitted** (`Slot::Admitted`), **Running** (`Slot::Running(Child)`), **Taken** (`Slot::Admitted` again after `take`), **Cancelled** (`Slot::Cancelled`, id still occupied).
Invariants claimed by the code: (a) only one call per id while the call runs ("a call with this id is running", mod.rs:84); (b) a cancelled call gets no response; (c) a cancel either prevents the spawn or kills that call's own child (calls.rs:33-34).

| from | event | to | where |
|---|---|---|---|
| Absent | tools/call with id, `admit` | Admitted | calls.rs:22-30, mod.rs:81-88 |
| Admitted/Running/Taken | duplicate tools/call with same id | unchanged, -32600 | calls.rs:30-31, mod.rs:82-86 |
| Admitted | `start` spawns child (under lock) | Running | calls.rs:35-43 |
| Absent/Cancelled | `start` | unchanged, returns "Cancelled" | calls.rs:42-46, call.rs:35-36 |
| Running | pipes hit EOF, `take` | Taken (child moved to call thread) | calls.rs:46-48, call.rs:44 |
| Admitted/Running/Taken | `notifications/cancelled` | Cancelled (child, if any, handed to `terminate` thread) | calls.rs:69-77, mod.rs:129-133 |
| Cancelled | tools/call with the same id | unchanged, -32600 | calls.rs:30-31 |
| Cancelled | `finish` | Absent, no response | calls.rs:80-84 |
| Admitted/Taken | `finish` | Absent, response written | calls.rs:59-61, mod.rs:141 |
| Absent | `finish` | Absent, no response | mod.rs:141-142 |

### SM2. MCP server loop / worker pool
States: **Reading**, **Draining** (stdin EOF, joining `running`), **Exited**. Worker count bounded by `MAX_CONCURRENT_CALLS` counting live threads (mod.rs:76-80). Non-`tools/call` requests and cancellations run inline on the reader thread (mod.rs:72-75), so a cancel is always ordered after the admit of the call it names. There is no initialize/initialized/shutdown state: the server is stateless by design (`initialize` only echoes a version, mod.rs:206-217; `notifications/initialized` falls into `respond` and returns at mod.rs:135-137; there is no `shutdown` method; EOF is the shutdown). Not reported as a bug.

| from | event | to | where |
|---|---|---|---|
| Reading | tools/call, pool full | Reading, busy result (id not admitted) | mod.rs:77-79 |
| Reading | tools/call admitted | Reading + 1 worker | mod.rs:89 |
| Reading | stdin EOF / read error | Draining | mod.rs:52, 103-107 |
| Draining | all workers joined | Exited (detached `terminate` threads die with the process) | mod.rs:91-94, calls.rs:54 |

### SM3. Batch run (`batch::execution::execute`)
Per entry: **Pending** → one of **NotStarted{deadline | session_ended | pre_action_baseline_failed}** or **Completed{ok | error | oversized→error}**. Run-level: `stopped ∈ {None, deadline, stop_on_error, pre_action_baseline_failed, output_limit}`; `pending_baseline` (captured before entry i for a `wait --event` at i+1, consumed at i+1).
Invariants: entries run in order; `completed + not_started == total` (not-started includes entries never reached); a baseline-capture failure stops before entry i runs.

| from | event | to | where |
|---|---|---|---|
| Pending(i) | deadline expired before start | NotStarted(deadline), stop | execution.rs:34-43 |
| Pending(i) | item session manifest has `ended_at` | NotStarted(session_ended); stop iff stop_on_error | execution.rs:47-59, 214-220 |
| Pending(i) | next is `wait --event`, baseline capture fails | NotStarted(pre_action_baseline_failed), stop | execution.rs:61-92 |
| Pending(i) | dispatch | Completed(ok/error) | execution.rs:97-116 |
| Completed(i) | entry exceeds remaining budget | Completed(error, result_omitted), stop output_limit | result_entry.rs:16-41, execution.rs:118-121 |
| Completed(i) error | stop_on_error | stop | execution.rs:122-125 |
| Completed(i) | deadline expired and i+1<total | stop (after_index) | execution.rs:126-129 |
| any | final body > 4 MiB | whole batch Err (uncertain if any completed) | execution.rs:156-173 |

### SM4. Dispatch bracket (`dispatch_command`)
Sequence: CommandScope acquired (session `ensure_active`, `command.start`) → overlay **Hidden** (headed + mutating + overlay enabled) → command → overlay **Shown** → `scope.complete` (`command.end`; a strict-trace failure replaces the result). `session end` uses `session_end_scope` (allows ended sessions). dispatch/mod.rs:52-142; core context/mod.rs:194-221, session_scope.rs:34-43. Show after an in-command Disable is a no-op on all adapters (Show never spawns a renderer), so `session end` inside the bracket is safe.

### SM5. Session start with cursor (two persisted steps)
`start_session` writes manifest (overlay disabled) → `set_cursor_overlay` rewrites manifest (overlay enabled) → adapter Enable. dispatch/session.rs:159-169, 185-221.

### SM6. Visual debug artifact file
**Absent** → `create_new` 0600 (visual_debug/mod.rs:56-65) → **Empty** → `finish` writes HTML (mod.rs:127-131) → **Written**. A path can be used once.

---

## C ABI (FFI)

Paths are relative to `crates/ffi/src/`.

### 1. Adapter handle (global `ADAPTERS: Mutex<HashMap<usize, Arc<AdAdapter>>>`, adapter.rs:16)

States: Unallocated → Live (id in map, Arc refcount ≥ 1) → Revoked (id gone from map, in-flight calls still hold `Arc`) → Dropped (last `Arc` released, platform adapter and session liveness lease dropped).

Invariants: ids come from a monotonic counter and are never reused (opaque_id.rs:5-19), so a stale token can't alias a new adapter. Each call holds an `Arc` for its whole body (`acquire_adapter!`, adapter.rs:66). Destroy only revokes new calls.

| From | Event | To | Where |
|---|---|---|---|
| Unallocated | `build_adapter` ok, `session_lease::acquire` ok, `register_adapter` | Live | adapter.rs:130-158, :36-45 |
| Unallocated | build, lease, or register error | Unallocated (null + last-error) | adapter.rs:131-157 |
| Live | `lookup_adapter` (any ad_* call) | Live (+1 Arc for the call) | adapter.rs:56-64 |
| Live | `ad_adapter_destroy` with no in-flight call | Dropped, **inside the ADAPTERS lock** | adapter.rs:224 |
| Live | `ad_adapter_destroy` while a call is in flight | Revoked | adapter.rs:224 |
| Revoked | in-flight call returns (last Arc) | Dropped (outside the lock) | Arc drop |
| Revoked/Dropped | new call with the old token | error `ErrInvalidArgs` | adapter.rs:58-63 |
| Revoked/Dropped | destroy again | no-op + last-error | adapter.rs:224-229 |

### 2. Native handle (thread-local `HANDLES`, actions/native_handle.rs:19-21)

States: None → Registered{owner_adapter_id, Rc<NativeHandle>} on the resolving thread → Released (record removed, or thread exit runs the TLS destructor).

Invariants: the token is a monotonic id, never an address (native_handle.rs:82-94). A call only works on the resolving thread, with the same adapter id (:96-119). Calls in flight hold an `Rc` clone, so a reentrant free can't drop the payload while it's in use. The handle is **not** tied to adapter liveness: it survives `ad_adapter_destroy` until `ad_free_handle` runs or the thread exits (test `handle_can_be_freed_after_adapter_destruction`, native_handle.rs:200-209).

| From | Event | To | Where |
|---|---|---|---|
| None | `ad_find_exact` success | Registered | observation/find.rs:130-133, native_handle.rs:84 |
| Registered | `ad_free_handle(same adapter id, same thread)` | Released, caller's `ptr` zeroed | native_handle.rs:63-67, :128-130 |
| Registered | free from a wrong thread or wrong adapter | Registered (error, nothing consumed) | native_handle.rs:131-134 |
| Released (`ptr` zeroed) | free again | no-op | native_handle.rs:60-61 |
| Registered | `ad_adapter_destroy(owner)` | **Registered (unchanged)** | adapter.rs:220-231 (doesn't touch HANDLES) |
| Registered | thread exit | Released (TLS drop) | native_handle.rs:19 |

### 3. Owned output allocations (global `ALLOCATIONS` registry, resource.rs:16)

States per (kind, addr): Registered(len) → Taken (freed). Every free path (`free_c_string`, convert/string.rs:40-47; tree, tree/free.rs:8-17, :54-70; action result, actions/result.rs:84-96, :153-183; notification actions) frees only after `take_allocation` returns `Some`, and nulls the field. So a double free of the same struct, a mutated count, or a foreign pointer is a no-op. The outer list boxes (`ad_*_list_free`, e.g. windows/list.rs:93-102) and `ad_image_buffer_free` (screenshot/free.rs:10-17) are not registry-backed: a double free there is documented UB.

### 4. Thread-local last-error (`LAST_ERROR: RefCell<Option<StoredError>>`, error.rs:80-82)

States: Empty → Set(code, msg…), replaced only by the next `set_last_error*` (error.rs:84-91). Invariant (doc, error.rs:200-217): a failing call returns the same code it stores. Most failure paths implement that as `set_last_error(&e); return last_error_code();`, which re-reads TLS, and `last_error_code` maps "unreadable" to `Ok` (error.rs:176-185).

### 5. Log callback (global `CALLBACK: Mutex<Option<fn>>`, log_callback.rs:21; thread-local `IN_CALLBACK`, :24-26)

States: Unset ↔ Set(fn), process-wide (`ad_set_log_callback`, :132-138). Per call, `with_dispatch` installs a scoped dispatcher only if a callback is set at entry (:115-122). Each event copies the fn pointer out of the mutex and calls it after releasing the lock (:71-82). `IN_CALLBACK` blocks recursion.
