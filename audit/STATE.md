# STATE: persisted and cross-process state other than refmaps

Repo /home/george/Projects/agent-desktop, main = upstream/main = b43ac4f6 (`git rev-list main..upstream/main` = 0). Nothing below is
already fixed on upstream/main, so no finding is DROPPED. Upstream #1-#8 fixed on 1170bcbc are skipped (overlap noted).
Line numbers are per file on main. Read-only audit: no build or test was run. Every "fails today" claim comes from reading the code.
The macOS, Windows and Linux renderer sections (STATE-13..16) come from a delegated read-only sub-audit. I re-checked F3 and F6 myself
(endpoint.rs:17-30/38-49/229-242, PROTOCOL_VERSION="v2" at endpoint.rs:6; spawn.rs:107-109). I did not re-trace the rest line by line.

Prior-art method: `scratchpad/pa.sh "<kw>"` runs `gh issue list` and `gh pr list --repo vercel-labs/agent-desktop --state all --search`,
`gh pr list --repo gshost1/agent-desktop --state all --search`, and `git log upstream/main -i --grep`. I also checked `git branch -r`
(there are no perf/ branches on origin, only origin/main). Raw output is in scratchpad/pa1.txt through pa5.txt.

Summary table

| id | name | class | status | upstream # |
|---|---|---|---|---|
| STATE-1 | FFI last-error code is re-read from TLS | B | CONFIRMED | #9 |
| STATE-2 | ad_adapter_destroy drops the adapter under the ADAPTERS lock | C (B if reentrant) | CONFIRMED | #14 |
| STATE-3 | Log-callback scope and native-handle revocation docs disagree with the code | D | CONFIRMED | #20 |
| STATE-4 | `mcp --debug --screenshot` makes every call after the first fail | B | CONFIRMED (worse than the register says) | #11 |
| STATE-5 | `session start --cursor` failure loses the session id and leaks an active session | B | CONFIRMED | #12 |
| STATE-6 | Manifest-less session directories are invisible to GC | B | CONFIRMED | #15, PR #498 |
| STATE-7 | Trace reader pairs command.start/end by name | B | CONFIRMED | #16 |
| STATE-8 | Pre-action screenshot is orphaned when the lease fails or the deadline passes | B | CONFIRMED | #18 |
| STATE-9 | Inherited interaction-lease fd: siblings, including concurrent MCP calls, are not excluded | A (if reached) | CONFIRMED mechanism, SUSPECTED in practice | new; #160, PR #590 |
| STATE-10 | Trace write ledger: fsync and a 50 ms lock per event drop events under contention | B + C | CONFIRMED mechanism, SUSPECTED rate | new; overlaps #4 |
| STATE-11 | One command reads the session manifest 4 times; one SessionSnapshot would remove the TOCTOU class | C/D | CONFIRMED | overlaps #2 |
| STATE-12 | Hide/Show is a switch, not a count | A (visual) | CONFIRMED | #13 |
| STATE-13 | Stale renderers from the other routing mode get re-shown; multi-agent Hide/Show carry no agent id | A (visual) | CONFIRMED | new; #175, #180 |
| STATE-14 | macOS legacy socket hash collides (`X`+`v2` == `Xv2`), so `session end` errors | B | CONFIRMED | new |
| STATE-15 | Renderer dead or not spawned: what actions report (macOS Ok-on-refused spawn, startup stall, Linux window leak after gc) | B | CONFIRMED | new; #385, #422 |
| STATE-16 | Batch items snapshot the overlay config at prepare and respawn a renderer the batch just disabled | B (visual) | CONFIRMED by chaining | new |
| STATE-17 | ProcessLeaseGuard is not reentrant: same-thread re-acquire stalls to TIMEOUT | B | SUSPECTED reachability | new |
| STATE-18 | FFI allocation registry is address-keyed (ABA on a stale struct copy) | B (memory safety, misuse) | SUSPECTED | new |
| STATE-19 | Small ledger entries: liveness leftovers, case-insensitive ids, lock identity by path, TraceWriterState, meta_written, artifact budget | D | — | — |

---

### STATE-1: FFI last-error code is re-read from TLS (upstream #9)
- Pieces: crates/ffi/src/error.rs:80-82 (`LAST_ERROR` thread_local), :84-91 `replace_last_error` (silently no-op when `try_with` fails or
  `try_borrow_mut` fails), :176-185 `last_error_code` (maps "TLS gone" and "borrowed" to `AdResult::Ok`). There are 91 call sites of
  `set_last_error(&e); return last_error_code();`, e.g. adapter.rs:71-72 (`acquire_adapter!`), operation.rs:49-50 and :61-62, adapter.rs:246-247.
- States: Empty -> Set(code). The invariant "a failing call returns the code it stored" holds only while TLS is alive.
- Illegal transition: failing call -> TLS destroyed -> `replace_last_error` drops the error -> `last_error_code()` returns Ok -> the
  caller sees AD_RESULT_OK for a failure. Reachable when a host calls any ad_* fn during thread teardown after Rust's TLS dtor for
  LAST_ERROR has run (C++ thread_local dtors, pthread key dtors, Swift/ObjC autorelease at thread exit). CONFIRMED for the code path;
  in practice it happens only at thread exit.
- Test sketch (fails today), in crates/ffi/src/error_tests.rs: spawn a thread. Inside it, first touch a custom `thread_local!{static G: Guard}`
  (registered first, so destroyed last), then call `set_last_error_static(...)` so LAST_ERROR registers its dtor second. In `Drop for Guard`, call
  `ad_check_permissions(std::ptr::without_provenance(usize::MAX))` and store the result in a static AtomicI32. After join, assert
  `!= AdResult::Ok as i32`. On Linux, dtors run in reverse registration order, so LAST_ERROR is gone first. Today the call returns Ok.
- Hidden cost: none.
- Proposed form: `set_last_error(&e) -> AdResult` returns the mapped code, and a `fail!(e)` macro returns it. `last_error_code()` is kept
  only for `ad_last_error_code`. S in logic, M in diff (91 mechanical sites). Pure behavior fix; no refactor needed.
- Overlap: #9. Prior art: none found. Searches: "last_error", "last error thread local", "ad_last_error_code", "AD_RESULT_OK failing call".
  The only hit is #67 (log callback introduction, unrelated).

### STATE-2: ad_adapter_destroy drops the adapter under the global lock (upstream #14)
- Pieces: crates/ffi/src/adapter.rs:16 `ADAPTERS`, :224 `if lock_adapters().remove(&id).is_none()`. Both temporaries (the MutexGuard
  and the `Option<Arc<AdAdapter>>`) live to the end of the `if` condition. The Option is created later, so it drops first, while the guard
  is still held. AdAdapter (:19-23) drops `Box<dyn PlatformAdapter>` (Linux: `SharedBus` with a zbus connection, linux/src/system/bus/mod.rs:156)
  and `SessionLivenessLease` (core session/liveness.rs:27-31: close plus unlink).
- Illegal transition: Live -> Dropped inside the critical section. Every concurrent `lookup_adapter` (adapter.rs:58) on any thread
  blocks behind the platform teardown. If tracing fires during the drop while a host log callback is installed (trap_panic_void ->
  with_dispatch, log_callback.rs:115-122), and the callback calls any adapter-taking ad_* fn on the same thread, `lock_adapters()`
  re-locks a std Mutex the thread already holds, which deadlocks. CONFIRMED (drop under lock); deadlock SUSPECTED (needs a drop path that emits tracing).
- Test sketch (fails today), adapter_tests.rs: register (via `registered_adapter`) an adapter whose `inner` type's `Drop` records
  `adapters().try_lock().is_ok()` into a static AtomicBool. Call `ad_adapter_destroy`, then assert true. Today it is false.
- Hidden cost: other threads stall for the length of the zbus connection drop plus one unlink (estimate: ms; not measured).
- Proposed form: `let removed = lock_adapters().remove(&id); if removed.is_none() {..}; drop(removed);`. S, behavior only.
- Overlap: #14. Prior art: none found. Searches: "ad_adapter_destroy", "adapter destroy lock" (hits #26, #117, #114, all unrelated).

### STATE-3: Log-callback scope and native-handle revocation docs disagree with the code (upstream #20)
- Pieces: log_callback.rs:21 `CALLBACK` is a process-wide `Mutex<Option<fn>>`. :124-125 and include/agent_desktop.h:1747-1748 say "inside later
  ad_* calls on the same thread". :71-82 copies the fn and calls it after unlocking, so an unregister races with in-flight events.
  threading.md:82-84 states this honestly, but the header does not. threading.md:58-60 says destroy "revokes native handles created by that
  adapter". adapter.rs:220-231 never touches `HANDLES` (actions/native_handle.rs). Handles become unusable only because `lookup_adapter`
  rejects the adapter. The retained platform object (AXUIElement or COM) stays alive until `ad_free_handle` or thread exit;
  ownership.md:93-99 says this correctly.
- Illegal transition: doc-only. Thread B, which never registered, receives callbacks; a destroyed adapter's handles are still allocated. CONFIRMED.
- Test sketch: thread A calls `ad_set_log_callback(cb)`. Thread B calls a failing ad_* that emits `tracing::warn!`. Assert cb saw 0 calls
  from B. That is what the header says should happen, and it fails today. Better: fix the docs.
- Proposed form: docs only (header, threading.md:58-60). S.
- Overlap: #20. Prior art: #71 and #67 introduced the callback. Searches: "log callback", "ad_set_log_callback thread", "native handle destroy revoke".
  No issue about the mismatch.

### STATE-4: `mcp --debug --screenshot` makes every call after the first fail (upstream #11)
- Pieces: src/mcp/mod.rs:243-259 `forwarded_globals` forwards every global flag set on the command line, including `--debug` and
  `--screenshot=PATH` (visual_debug/options.rs:17-31, both `global = true`). src/mcp/call.rs:29-32 prepends them to every child.
  src/main.rs:159 runs `cli.visual_debug.validate(&cmd)?`, and options.rs:39-43 rejects every command other than snapshot or click.
  visual_debug/mod.rs:56-65 uses `create_new(true)`, so the path can be used once.
- States: path Absent -> Created (first snapshot/click) -> every later child gets "Cannot create debug output" (INVALID_ARGS).
  All other tools (find, type, wait, ...) fail from the very first call with "--debug supports snapshot and click only".
  The register says "every snapshot or click after the first". In fact the whole server is unusable except for one call. CONFIRMED.
- Test sketch (fails today), src/mcp/mod_tests.rs: build matches for `["agent-desktop","--debug","--screenshot","/tmp/x.html","mcp"]`
  and assert `forwarded_globals(&m)` contains neither `--debug` nor `--screenshot`. Alternatively, `mcp::serve` must refuse to start with them.
- Proposed form: mark the debug flags as non-forwardable (an allowlist of forwardable globals) and have `mcp` refuse them up front. S.
- Overlap: #11; mcp/mod.rs is also touched by open PR #587 (read-only server mode). Prior art: none found. Searches: "mcp debug screenshot",
  "visual debug create_new", "--screenshot html".

### STATE-5: `session start --cursor` failure loses the session id and leaks an active session (upstream #12)
- Pieces: src/dispatch/session.rs:18-24 (the session is created, then `show_default_cursor(...)?`). :57-60 `cursor_overlay::execute(...)?` is the
  only fallible step: a manifest read or write fails, or the manifest is unreadable. An adapter failure is swallowed into `rendered:false` (:70-76).
  core session/gc.rs:72-74: without `--older-than`, GC skips sessions with `ended_at = None`.
- Illegal transition: Active session created -> error envelope without `session_id` -> the caller cannot `session end` it. It stays Active
  (trace On) and is reclaimed only by `gc --older-than`. CONFIRMED (path); the trigger is a filesystem fault.
- Test sketch (fails today), src/dispatch/session_tests.rs (it is a child module, so it can reach the private fn): under `HomeGuard`, call
  `show_default_cursor(&NoopAdapter, &mut json!({"session_id":"run-x"}), false)` for a session that was never started. Assert that the error's
  `details.session_id == "run-x"`. Today the details are absent.
- Proposed form: wrap the error with `details.session_id` and a suggestion to run `session end <id>`, or end the session on failure. S.
- Overlap: #12. Prior art: none found. Searches: "session start --cursor" (hits #422, #385, #464, #175, #179: renderer bugs, not this one),
  "show_default_cursor".

### STATE-6: Manifest-less session directories are invisible to GC (upstream #15)
- Pieces: core session/mod.rs:271-297 `list_sessions` skips directories whose `read_manifest` is None (:291). That includes unreadable
  manifests, which read_manifest collapses to None (:85-109). session/mod.rs:313-324: `tree::create`, then `write_manifest`. liveness.rs:40-44:
  `base.is_dir()`, then `RefStoreLock::acquire_with_deadline`, which opens with create and recreates the directory through
  private_file_parent::ensure_directory_path. gc.rs:87 `remove_session_dir(&dir)?` aborts the whole loop on the first error, and
  `remove_dir_all` (:135) may already have deleted `session.json`. session/mod.rs:197-212 `save_cursor_overlay_profile` writes
  `cursor-overlays/<agent>.json` with no lock, so it can recreate a reaped directory.
- States: Absent -> Bare (directory, no manifest) -> stuck. Bare is a legal state by design (CONCEPTS.md), but no transition leaves it.
- Reachable: (a) a crash between session/mod.rs:314 and :324; (b) GC removes the directory between liveness.rs:40 and :43, so the lock open
  recreates `sessions/<id>/refstore.lock`; (c) a partial `remove_dir_all` error, which also skips every remaining session; (d) any `--session <typo>`
  writer; (e) a profile save racing GC. CONFIRMED for (b), (c) and (e) by reading.
- Test sketch (fails today), session_gc_tests.rs: create `sessions/run-orphan/refstore.lock` with no manifest, set mtimes 1 h old, run
  `gc(GcOptions{ended_only:false, older_than:Some(Duration::ZERO)})`, and assert the directory is gone. Second test: make one session's
  subdirectory undeletable (chmod 0500 on `trace/`). Assert gc still removes a second eligible session and reports the failure, instead of returning Err.
- Proposed form: a `SessionDirState { Bare{age}, Active, Ended, Unreadable }` classification in gc, with Bare and Unreadable reclaimed after
  an age. Collect per-session errors instead of `?`. Pass `ensure_directory_path(create=false)` for lock opens in liveness::acquire. M.
- Overlap: #15. Open PR #498 also edits gc.rs: it re-reads the manifest under the lock because `--older-than` uses the stale `list_sessions`
  manifest (gc.rs:58 vs 66-71). That is the same "decide on a stale snapshot" class. Prior art: #601 (open, lifecycle races), PR #498 (open).
  Searches: "session gc manifest", "orphan session directory", "remove_session_dir".

### STATE-7: Trace reader pairs command.start/end by name (upstream #16)
- Pieces: core trace/trace_read/merge.rs:95-155, key = `segment:writer_pid:command` (:116). `open.insert` overwrites (:119). The segment name
  is process-wide (trace_file.rs:5-17). FFI per-call scopes: ffi/src/commands/snapshot.rs:113, wait.rs:75, status.rs:61, execute_by_ref_timeout.rs:90.
- Illegal transition: two FFI host threads run `snapshot` on one session at overlapping times: start A, start B (overwrites A), end A
  (removes), end B -> false "Unpaired command.end". The reverse also happens: start A, start B, end B, A crashes -> A's lost end is hidden.
  CONFIRMED. CLI and MCP are not affected (one pid per command).
- Test sketch (fails today), trace_read/merge_tests.rs: four events with the same segment, pid and command: start, start, end, end. Assert
  `detect_unpaired_commands(&events, None).is_empty()`. Today it returns one warning.
- Proposed form: carry `command_seq` (the seq of its start) in command.end (context/mod.rs:61-71) and pair by it. A per-key count is the S
  fallback for older traces. S.
- Overlap: #16. Prior art: none found. Searches: "unpaired command" (#91, the viewer introduction), "detect_unpaired_commands".

### STATE-8: Pre-action screenshot is orphaned when the lease fails or the deadline passes (upstream #18)
- Pieces: core ref_action/ref_action_wait.rs:91-96 capture `pre`, :99-102 lease `?` returns, :103-106 expired returns. Neither path calls
  `finish_artifacts` (ref_action/mod.rs:217-236). Only the refusal path does (:123). The screenshot is already written and counted in
  `.artifact-usage.json` (trace/trace_artifact_budget.rs:71-97).
- Illegal transition: PreArtifact(written) -> Failed with no `action.artifacts` event. The file in trace/screens is never referenced, and it still
  consumes the artifact byte and count budget. CONFIRMED.
- Test sketch (fails today): a stub adapter whose `acquire_interaction_lease` returns `AdapterError::timeout`, whose `capture_screenshot`
  returns a 1x1 image, and a Full-artifacts session under HomeGuard. Run a ref click with an auto-wait timeout. Assert every file in
  `trace/screens/` is referenced by some trace event. Today one is not.
- Proposed form: a `PreArtifact` guard whose Drop (or an explicit `abandon(context)`) emits the pre-only artifact event. Alternatively, move
  capture after the lease. S.
- Overlap: #18; ref_action_wait.rs is also on the perf path (pre capture before the lease). Prior art: none found. Searches:
  "pre artifact screenshot lease", "orphan screenshot trace".

### STATE-9 (new): Inherited interaction-lease fd does not exclude siblings, including concurrent MCP calls
- Pieces: core session/interaction_lease.rs:122-174 adopt -> storage/file_lock.rs:54-89 `adopt_inherited`. It dups the fd, sets CLOEXEC
  on the raw fd but never closes it, checks dev/ino, then `lock_file` -> `try_lock` on the inherited open file description (OFD). flock on an
  OFD that is already locked through that same OFD always succeeds. Callers: linux/src/system/interaction/interaction_lease.rs:10-21,
  macos/src/system/adapter.rs:28-53, ffi/src/operation.rs:9-16. src/mcp/call.rs:29-35 spawns children with the inherited environment and
  fds (no `env_remove(INTERACTION_LEASE_FD_ENV)`). The MCP server never adopts, so the fd stays inheritable. Concurrency is up to
  `MAX_CONCURRENT_CALLS = 8` (mcp/mod.rs:20).
- States: Free | Held(OFD). Adoption means "same OFD", so any number of processes holding that OFD are all "holders" at once. Release
  happens only when every copy closes. The child keeps its raw copy until exit (file_lock.rs:67-73).
- Illegal transitions: (1) `agent-desktop mcp` started by a lease-holding runner -> two concurrent mutating tool calls both adopt -> both
  dispatch input at the same time (impact A). (2) A long-lived FFI host started with the env set keeps the lock alive after the parent
  releases, so every other agent-desktop process on the machine gets TIMEOUT (B). CONFIRMED by code; whether real runners do this is SUSPECTED.
- Test sketch (fails today), interaction_lease_tests.rs subprocess pattern (like session_gc_tests.rs `subprocess_session_lease_holder`):
  the parent locks the canonical file and spawns two child test processes with the fd and env. Child 1 adopts and holds for 500 ms. Child 2
  adopts with a 100 ms deadline and must get TIMEOUT. Today it succeeds. A narrower test: assert that the Command built by mcp `call` has the env var removed.
- Proposed form: `InteractionLeaseSource { Owned, Inherited { exclusive_token } }`. A parent-issued token is consumed once (for example,
  the child takes a second flock on a per-child file the parent created). At minimum, strip the env var and the fd in mcp call.rs and close
  the raw fd after adoption. S for the MCP strip, M for real exclusivity.
- Prior art: #160 (closed, "E2E harness interaction lock is silently released after first inherited-lease operation", which led to close-not-unlock),
  PR #590 (open, moves the lock directory and removes the FFI duplicate inherited path, ffi/src/operation.rs). Neither covers sibling
  exclusion. Searches: "interaction lease inherited", "INTERACTION_LEASE_FD".

### STATE-10 (new): Trace write ledger: fsync and a 50 ms lock per event drop events under contention
- Pieces: core trace/mod.rs:84-95 `TraceWriteBudget::acquire` runs on every `emit`. trace_write_budget.rs:23-49: `ensure_trace_dir`, then lock
  `.trace-budget.lock` with `Deadline::detached_after(50)`, read the ledger, stat the segment. :51-55 `reserve_line` -> `persist` (:57-72):
  `set_len(0)`, write, `sync_data()`. That is one fsync per trace line; on macOS `sync_data` is F_FULLFSYNC. On the upstream fix branch 1170bcbc,
  persist becomes `write_private_file` = `write_atomic` (sync_all, rename, parent dir fsync: storage/private_file.rs:127-134), which means 2 full syncs per line.
- Illegal transition: under contention, waiting longer than 50 ms makes `acquire` Err. Non-strict mode drops the event silently
  (mod.rs:89-92 `return Ok(())`), so a missing command.start or command.end shows up as a false reader warning (STATE-7) and a lost record.
  Strict mode turns a delivered action into an error through `trace_error_with_disposition` (context/mod.rs:53-58). It is reached by N parallel
  writers in one session (MCP with up to 8 calls, batch plus FFI) when each lock hold includes a full sync. CONFIRMED mechanism; drop rate SUSPECTED (needs probe).
- Hidden cost (estimate, not measured): per event 1 mkdir-stat, 2 opens, 1 flock, 1 read, 1 stat, 1 truncate, 1 write, 1 fsync. A typical
  command emits 2-4 events, so 2-4 F_FULLFSYNC per command, roughly 5-20 ms each on Apple SSDs, which is 10-80 ms per command. The
  fix branch doubles that.
- Test sketch (fails today), trace_tests.rs: hold `.trace-budget.lock` from another thread for 80 ms, call `TraceConfig::emit` (non-strict),
  and assert the event lands once the lock frees. Today it is silently dropped after 50 ms.
- Proposed form: a `TraceBudgetReservation` taken once per CommandScope (reserve K events or B bytes up front, settle at complete). Do not
  fsync the ledger: on crash, rescan with `initial_usage`, which already exists, instead of trusting a synced ledger. Behavior fix S;
  reservation refactor M.
- Overlap: #4 (the fix branch edits the same persist). Perf work on trace emission would touch trace/mod.rs and trace_write_budget.rs.
  Prior art: #600 (open, killed-writer ledger, which is #4). No issue on per-event fsync or the lock budget. Searches: "trace budget lock",
  "trace-usage", "trace event dropped", "fsync trace".

### STATE-11 (new): One command reads the session manifest 4 times
- Pieces (CLI command with `--session`): context/mod.rs:92-93 `session_trace_state` read #1 (:351). :94 lease: liveness.rs:43-44 lock plus read #2.
  :95-96 `cursor_overlay_for_session` read #3 (session/mod.rs:144). :197-198 `ensure_active` read #4 (session_scope.rs:18). With
  `--agent-id`, add a profile read. Batch adds `session_ended_for` per item (src/batch/execution.rs:214-220). Each value is decided
  independently, so trace-on, overlay-on and ended can come from different manifest versions.
- Illegal transitions: #2 (fixed on the branch by reordering) is one instance. Others are harmless today (an overlay toggled between
  reads #3 and #4). CONFIRMED.
- Hidden cost (estimate): 3 extra open, read and serde round trips of a file of at most 64 KiB per command, under 1 ms. C/D.
- Proposed form: `liveness::acquire` already holds the refstore lock and reads the manifest, so it should return
  `SessionSnapshot { id, lease, manifest: Option<SessionManifest> }`, and trace state, overlay and ensure_active derive from that one value. This makes
  #2-style ordering bugs unrepresentable. M (context/mod.rs, session_scope.rs, liveness.rs, session/mod.rs). Pure refactor; can land after #2.
- Overlap: #2. Prior art: #601 and PR #496 (open, session end vs overlay update). Searches: "manifest read", "liveness lease".

### STATE-12: Hide/Show is a switch, not a count (upstream #13)
- Pieces: src/dispatch/mod.rs:52-57 decides `overlay_session`, :67-69 sends Hide before the command, :138-140 sends Show after it. No session-wide
  serialization exists between mutating commands (context/mod.rs:180-213). The per-cursor lock (cursor_overlay/cursor_hold.rs) is taken inside
  the command and released before Show. Renderers toggle: macOS cursor_overlay_bridge.m:302 (hide), :362 (show), and every Present re-fronts (:406);
  Windows surface_host.rs:85-91; Linux cursor_overlay_draw.rs:107-113 and 132-136.
- Illegal transition: A Hide, B Hide, A acts and returns (lease and cursor lock released), B acquires and dispatches, then A's Show arrives -> the
  overlay is visible during B's headed input. CONFIRMED. Note: in headed ref actions, travel still runs under the lease (ref_action/presentation.rs:14-36),
  and every Present re-shows the renderer on macOS and Linux. So Hide is already undone by the action's own travel there. The race
  matters most for Windows "suppressed for headed input" and for non-travel commands (press, type, clipboard). On macOS, an agent-less Hide/Show is a broadcast
  that takes the machine-wide `.cursor-overlay-start.lock` for up to 4 s (macos spawn.rs:60-76): this is C cost per headed command.
- Test sketch (fails today), src/dispatch/cursor_overlay_lifecycle_tests.rs: a blocking adapter holds command B mid-action on thread 2 while
  command A completes on thread 1 (both use the `started_overlay_session()` session). A recording `update_cursor_overlay` must not see Show
  while B is in flight. Today it records Hide, Hide, Show.
- Proposed form: a `HeadedSuppression` count under `sessions/<id>/cursor/headed.count`, kept as a set of lease files like liveness/ (a holder is
  live while its flock is held, so a crashed holder is not counted). Show is sent only when the set empties. Or the renderer keeps a count of
  named holders. M. Decide intent first (the register says "intent unconfirmed").
- Overlap: #13. Prior art: #178 (closed, overlay stays hidden after press/clipboard), PR #183 (merged, restored the lifecycle), PR #184, PR #528 (open, macOS
  windows covering the cursor). None add counting. Searches: "cursor overlay hide show", "headed overlay hide".

### STATE-13 (new): Stale renderers from the other routing mode get re-shown; multi-agent Hide/Show carry no agent id
- Pieces: src/dispatch/cursor_overlay.rs:44-50: Enable sends an Enable control in single mode and nothing in multi-agent mode. It never disables
  the other mode's renderers. dispatch/mod.rs:68,139 send `hide(session)`/`show(session)` with `agent_id: None`. Agent routing is in
  core cursor_overlay/submit.rs:223-229. Renderer liveness: macOS child.rs:350-361 `session_active(s, Some(agent))` stays true after
  multi->single. Windows session_state.rs:56-59 has no multi-agent check, and windows spawn.rs:51-63 spawns an agent-less renderer
  for a command without `--agent-id` in a multi-agent session (macOS refuses at spawn.rs:28-31). Linux cursor_overlay.rs:129-131 maps every
  window of the session.
- Illegal transitions: switching single->multi (Windows, Linux) or multi->single (macOS, Linux) leaves the old-mode renderer Live. The next
  headed command's agent-less Show un-rests it, so a ghost cursor is drawn at a stale point. Windows' agent-less Hide/Show reaches only the
  agent-less pipe, so in a multi-agent session headed commands never hide agent cursors. macOS and Linux broadcast, so agent A's Hide cancels agent B's
  drag trail (bridge.m:302-310). CONFIRMED (sub-audit, chains traced).
- Test sketch (fails today): macOS child_tests.rs: a manifest that is enabled and not multi-agent; assert `!session_active("s", Some("agent-a"))`.
  Windows session_state_tests.rs: an agent-less reader of `{"cursor_overlay":{"enabled":true,"multi_agent":true}}` must not classify as Live.
- Proposed form: a `CursorRouting { Single, Multi }` stored in the manifest, carried in every control, and checked by the renderer, which exits
  when the routing no longer matches. Enable in either mode sends a Disable for the other mode first. Hide/Show get explicit
  `Broadcast | Agent(id)` targeting. M across core control.rs and three adapters.
- Prior art: #175 (closed, `--agent-id` without `--multi-agent` spawns a second overlay), PR #180 (merged, gate agent_id stamping), PR #364 (closed),
  PR #170 (closed). None about a mode switch. Searches: "multi-agent cursor", "agent id collision", "agent-id case insensitive".

### STATE-14 (new): macOS legacy socket name collides across sessions
- Pieces: crates/macos/src/system/cursor_overlay/endpoint.rs:229-242. `endpoint_hash` FNV-hashes root, session_id and protocol bytes
  concatenated with no separator. `legacy_path(id)` = hash(root, id, None) (:17-30); `path(id, None)` = hash(root, id, Some("v2")) (:38-49, :6).
  So legacy_path("Xv2") == path("X"). discover (:147-181) includes the legacy path (:159). Session ids may end in "v2" (core context/mod.rs:361-366).
- Illegal transition: `session end Xv2` while session X has a live renderer. Discover returns X's socket, and X's child drops the
  mismatched-session Disable without acking (child.rs:77-83). The sender hits EOF (spawn.rs:293), and src/dispatch/session.rs:31 returns "cursor
  teardown was not confirmed" (disposition uncertain) even though Xv2's manifest was ended. I re-checked this chain myself (endpoint.rs lines above).
  CONFIRMED. Reachable only with user-chosen ids (generated `run-...-N` ids never end in "v2"). B.
- Test sketch (fails today), in endpoint.rs tests: `assert_ne!(endpoint_hash(root,"run-1v2",None), endpoint_hash(root,"run-1",Some("v2")))`.
- Proposed form: hash with a separator and a length prefix (`id.len()` as bytes, then a `\0`), or drop the legacy path. S.
- Prior art: none found (no issue on endpoint hashing; hits are unrelated overlay PRs #175, #396, #171). Searches: "cursor overlay socket", "legacy socket", "endpoint_hash".

### STATE-15 (new): Renderer dead or not spawned: who restarts it and what the action reports
- Q from brief, overlay enabled but renderer dead. Answer (sub-audit): nobody keeps it alive. The next non-transient control respawns it:
  macOS spawn.rs:49-57 under `.cursor-overlay-start.lock`, connect polled every 10 ms (:153-161), budget 900 ms for Present and 4000 ms otherwise (:33-37);
  Windows framing.rs:72-74 and spawn.rs:59-63, budget 900 ms, backoff 8->128 ms; Linux creates the window on any non-transient control
  (cursor_overlay.rs:170-176), 4000 ms under `.x11-cursor.lock`. The action itself reports nothing: core submit.rs:239-244 logs a warning
  and returns false. Only Enable surfaces `rendered:false`.
- Confirmed defects on this path:
  - (a) macOS returns `Ok(())` without spawning when the executable stem is not `agent-desktop` (spawn.rs:107-109; I re-checked this). FFI hosts and renamed binaries
    therefore get `rendered:true` with nothing drawn. Windows returns Err deliberately (windows spawn.rs:205-224). B, CONFIRMED.
  - (b) The macOS child draws the initial control before binding its socket (child.rs:45 before :48), and the parent never `try_wait`s. A child
    that dies at once (no display, off-screen destination) costs the full 900 or 4000 ms stall and then reports "did not become ready". A cold
    start plus travel of up to 700 ms can exceed 900 ms, so the child is killed mid-draw (spawn.rs:162-174). C and B, path CONFIRMED,
    kill loop SUSPECTED.
  - (c) The macOS renderer polls the manifest every 1 s and exits on unreadable, absent or ended alike (child.rs:63-66, 352-361), because core
    read_manifest collapses errors to None (session/mod.rs:92-96). One transient read error kills a live session's cursor. Windows
    separates Live, Finished and Unknown and needs 2 Finished readings in a row (session_state.rs:33-96). B, CONFIRMED.
  - (d) Linux overlay windows are `RETAIN_PERMANENT` (cursor_overlay_draw.rs:293) and `session gc` sends no Disable
    (src/dispatch/session.rs:35-38). A window from a reaped or crashed session stays on screen forever. B, CONFIRMED.
- Test sketches (fail today): (a) macos spawn_tests.rs: from the test binary, `spawn(&tmp_sock, &CursorOverlayControl::enable(..), deadline)`
  must be Err. (b) extract `await_ready(child, socket, deadline)`, feed it `Command::new("false")`, and assert it returns within 100 ms
  with an exit-status error. (c) child_tests.rs: point the state root at an unreadable manifest (mode 000) and assert `session_active` reports
  Unknown, not false. (d) dispatch test with a recording adapter: `session gc --older-than 0` on an overlay-enabled session must record
  `Disable{session}`.
- Proposed form: `RendererState { Absent, Starting{child, deadline}, Ready, Failed(reason) }` per endpoint in each adapter's spawn path,
  with `try_wait` in Starting. Use a core-provided `manifest_status(id) -> Live | Finished | Unknown` (lift Windows' session_state into core)
  that all renderers use. gc dispatch sends Disable for each removed id. M.
- Prior art: #385 (open, macOS 27 rendered:true but draws nothing), #422 (open, Windows never renders, rendered:false), PRs #396 and #456 (open,
  start overlay from packaged/npm binaries; these touch the same spawn gate as (a)), PR #381 (merged). Nothing for (d). Searches: "session start --cursor",
  "renderer died", "cursor overlay renderer restart", "linux overlay window gc".

### STATE-16 (new): Batch items snapshot the overlay config at prepare and respawn a renderer the batch just disabled
- Pieces: src/batch/preparation.rs:45-53 builds every item context up front through `for_batch_item` (core context/mod.rs:238-275). A same-session
  item copies `self.options.cursor_overlay` (:260-262). src/batch/execution.rs:47-59 re-checks only `ended_at`, not the overlay. Present
  respawns on macOS, Windows and Linux (STATE-15).
- Illegal transition: `batch '[["cursor-overlay","disable"],["click","@e1"]]'` (or a concurrent `cursor-overlay disable` in another process during a
  long command). The click's context still says enabled, so hold_cursor, travel and Present respawn the renderer. On macOS the spawn gate re-reads the manifest
  (spawn.rs:28-31) and refuses, so this hits Windows and Linux. The respawned renderer then exits on its own idle manifest check (Windows) or
  stays (Linux, STATE-15d). CONFIRMED by chaining (prepare snapshot, then the adapter Present-spawn rule); not run.
- Test sketch (fails today), src/batch/execution_tests.rs: an overlay-enabled session with a recording adapter. Run the batch
  [cursor-overlay disable, mouse-move 1,1]. Assert no `Present` control is recorded after the `Disable`.
- Proposed form: items read the overlay from the SessionSnapshot taken at dispatch (STATE-11), not at prepare. S once STATE-11 exists.
- Prior art: commit 899ce270 (branch detail/bug-fix/fix-skip-ended-session-batch-entries, merged into main: re-checks ended per item). Nothing for the overlay. Searches:
  "batch cursor-overlay disable".

### STATE-17 (new): ProcessLeaseGuard is not reentrant
- Pieces: core session/process_lease_guard.rs:6 global `AtomicBool`, :13-33 spin at 1 ms until the deadline. FFI lease deadline is 5000 ms (ffi/src/operation.rs:3).
- Illegal transition: a thread holding the interaction lease asks again: Held -> (same thread) acquire -> spins 5 s -> TIMEOUT
  `interaction_process_lock_timeout`. A reachable shape is a host log callback (called synchronously inside an action, log_callback.rs:82) that calls
  another mutating ad_* on the same thread. SUSPECTED (contrived). B.
- Test sketch: in a log callback installed by the test, call `ad_mouse_move` on the stub adapter while an outer `ad_mouse_click` holds the lease.
  Assert the inner call returns within 100 ms with a clear "reentrant" error. Today it takes 5 s and returns TIMEOUT.
- Proposed form: store an owner thread id in the guard, and fail fast with `ErrorCode::PolicyDenied{kind:"reentrant_interaction_lease"}`. S.
- Prior art: none found. Searches: "process lease reentrant".

### STATE-18 (new): FFI allocation registry is address-keyed (ABA on a stale copy)
- Pieces: ffi/src/resource.rs:16 `ALLOCATIONS: (kind, addr) -> len`, :31-41. Free paths null the field in the struct passed, but not in copies.
- Illegal transition: the host copies an AdActionResult by value (Swift structs, ctypes copies), frees copy 1, the next call's allocation reuses
  the address (likely for the same size class), then the host frees stale copy 2. The registry matches and frees the live allocation, so the host has a use-after-free.
  The docs call copying unsupported (header :1132-1135 for handles; ownership.md:113-115 "release the unmodified result struct").
  This is hardening, not a contract break. SUSPECTED.
- Proposed form: put a generation id in the registry and store it in an existing reserved field. That needs an ABI field, so it is L, or doc-only. Native
  handles already use monotonic ids (actions/native_handle.rs:82-94).
- Prior art: none found. Searches: "double free allocation registry", "take_allocation".

### STATE-19: Ledger-only entries (impact D unless noted)
- Liveness leftovers: a killed holder leaves `liveness/<pid>-<ns>-<seq>.lock` forever (liveness.rs:27-31 unlinks only on Drop), and `any_held` (:59-68)
  opens and flocks every file on each `is_live` and gc pass. This grows linearly with crashed commands per session. C, small.
- Case-insensitive ids (macOS APFS and NTFS default): `Agent-A` and `agent-a` pass `validate_agent_id` (session/mod.rs:222-230) as distinct ids. They get
  distinct macOS sockets (exact-byte hash) but share `cursor-overlays/<id>.json` (:214-220) and `cursor/agent-<id>.lock` (cursor_hold.rs:37-40).
  The result is one profile and serialized travel for two cursors. The same applies to user-chosen session ids sharing a directory. D, CONFIRMED by reasoning.
- Interaction lock identity is by path while flock is by inode: if `/tmp/agent-desktop-<uid>/interaction.lock` is unlinked while held
  (tmp cleaners), a new process locks a fresh inode and two holders coexist. Adoption then fails its dev/ino check with PolicyDenied. SUSPECTED, D.
  The directory-squatting DoS is open PR #590.
- InteractionLease variants (Guarded, FileHeld, Adopted) and `with_deadline` clones: no reachable illegal transition besides STATE-9 and STATE-17.
- TraceWriterState × TracePending (trace/mod.rs:27-61, 117-152): strict mode keeps Unopened and retries open on every emit. Otherwise the state
  ends Failed. Consistent; D.
- `meta_written` (trace/mod.rs:186-202): per TraceState, and the "file len > 0" check runs under flock, so FFI per-call states share one
  segment correctly. D.
- Artifact budget ledger (trace_artifact_budget.rs): documented skip on lock timeout; rollback when the file is absent. Over-counting on crash
  is deliberate. D.
- Session ended mid-command or mid-batch: `ensure_active` is checked only at scope start (context/mod.rs:197-199). A running command keeps
  tracing into the ended session, and batch's own command.end lands after an in-batch `session end`. GcOptions docs say this is intended. D.
- FFI direct ops (mouse, keyboard, clipboard, native handle actions) never build a CommandContext, so they bypass the ended-session refusal, trace and overlay.
  Only snapshot, wait, status, execute_by_ref and trace use it (ffi/src/adapter.rs:272-274). D, by design but undocumented.
