# REFS: snapshot -> refmap -> resolve -> STALE_REF / AMBIGUOUS_TARGET

Repo `/home/george/Projects/agent-desktop`, main = upstream/main = `b43ac4f6` (verified: `git log b43ac4f6..upstream/main` is empty).
Read-only audit. Paths relative to the repo; core paths relative to `crates/core/src/` unless prefixed.

## Upstream register, REFS-area items checked against main

| # | Status on main | Note |
|---|---|---|
| #3 | Still present on main (fixed only on unmerged `upstream/laf/state-machine-fixes` 1170bcbc) | `refs/refs_store_prune.rs:60` removes a snapshot dir if `trace/refmaps/<id>.json` merely *exists*; a drill rewrote `refmap.json` but the copy is write-if-absent (`trace/trace_artifact_budget.rs:105`), so end_session deletes the only copy of drilled refs. Not re-reported; feeds REFS-3. |
| #8 | Still present on main (fixed on the laf branch) | `session/mod.rs:330` `end_session` takes a liveness lease, not `refstore.lock`, around `discard_duplicated_ref_scaffolding`. Not re-reported. |
| #10 | **Still true, CONFIRMED, and wider than registered** | See REFS-5. Reachable only through `batch` (the only inherited-deadline setter besides `main.rs:199`, which always passes `None`). Also hits `find` and `snapshot --root`. |
| #9, #11-#20 | Outside REFS | Not checked here. |

Upstream "SM1 I2: latest names a Committed directory": false on main (REFS-1). "SM4: no invalid transitions": the attempt classifier is fine, but its consumers disagree (REFS-4).

---

### REFS-1: latest_snapshot_id pointer vs snapshots/ directory (status, legacy migration, pointer semantics)
- Pieces:
  - Pointer write: `refs/refs_store.rs:58-64` (`save_new_snapshot_unlocked_with_id`: save dir, set pointer, prune), `:193-196`.
  - Pointer read: `refs/refs_store.rs:154-156`, `:202-218`; sole production consumer `commands/status.rs:24-29`.
  - Dead legacy path: `refs/refs_store.rs:104-114` (`load_latest`), `:263-283` (`migrate_legacy_latest`), `refs/mod.rs:185-191,216-218` (`RefMap::load` of `last_refmap.json`). `RefStore::load(None)` / `load_latest` have no non-test caller (grep over `crates/` and `src/`).
  - Pointer drop: `refs/refs_store_prune.rs:64-69` (only after end_session discard).
  - Who moves it: snapshot (`snapshot/mod.rs:182`), find (`commands/find_live.rs:174-179`, even for zero matches), wait --wait-for (`commands/wait_selector.rs:207,301`). Who does not: drill (`snapshot/snapshot_ref.rs:110`, `update_existing_snapshot` never touches the pointer).
- States and transitions today:
  ```
  NoPointer --save_new_snapshot--> Points(id, dir present)
  Points(id) --save_new_snapshot--> Points(id')        (prune never removes the new latest)
  Points(id) --drill on any snapshot--> Points(id)     (pointer now names a map that is not the newest write)
  Points(id) --find with 0 matches--> Points(empty map)
  Points(id) --rm -rf snapshots/id | partial remove_dir_all | session gc of dir only--> Dangling(id)   [unhandled]
  Dangling(id) --status--> SNAPSHOT_NOT_FOUND, forever until next save
  NoPointer + legacy last_refmap.json --any production command--> NoPointer (never migrated)
  ```
- Illegal / unhandled transitions:
  1. **Dangling pointer fails `status`** (upstream issue #537, open). `status.rs:25` reads pointer, `:27` `load_snapshot(id)?` -> `refs_store.rs:127-128` -> `SNAPSHOT_NOT_FOUND` with the ref-action recovery text ("re-run a snapshot... Snapshots are scoped to the session"). Reached by agents that delete their own snapshot dirs for privacy (the #537 reporter does this on two Linux machines), or any external cleanup. Also any load error (undecodable map from an older build -> `INTERNAL` via `AppError::Json`, `response/app_error.rs:30`; an entry failing `refs_validate.rs` -> `INVALID_ARGS`) fails the whole health check. CONFIRMED (traced end to end).
     Test sketch (fails on main): in `commands/status_tests.rs`, `HomeGuard::new()`, `RefStore::new().save_new_snapshot(&RefMap::new())`, `remove_dir_all(<home>/snapshots/<id>)`, call `execute_with_report_with_context(&DeniedAdapter, &report, &CommandContext::default())`; assert `Ok` with `snapshot_id == null` (or a `snapshot_missing` flag) and `ref_count == null`.
  2. **Legacy migration is unreachable.** `migrate_legacy_latest` only runs from `load_latest`, which nothing in production calls, so a pre-snapshot-dir `last_refmap.json` is never migrated and `status` reports `snapshot_id: null` on such a store. Impact D (refs are qualified, so a legacy map was unusable anyway), but the code and its tests describe a transition that cannot happen. CONFIRMED.
  3. **Pointer meaning depends on which command ran last.** After `snapshot` then `find --role button --name Nope`, `status.snapshot_id` names an empty find map; after a drill, it names a map that is not the one just written. Status's `ref_count` is therefore not "the current snapshot's refs". Impact B-low (status only). CONFIRMED.
  4. Narrow race noted by upstream PR #500: pointer read and load are separate and unlocked; prune removes oldest-by-mtime, and with coarse mtime (HFS+/FAT 1-2 s) ties sort by random id (`refs_store_prune.rs:108`), so the previous latest can be evicted between status's two reads. SUSPECTED (needs probe on a coarse-mtime FS).
- Hidden cost: none.
- Impact class: **B** (wrong error from the health check, wrong recovery text).
- Proposed explicit form: `enum LatestSnapshot { Absent, Present { id, map }, Dangling { id } }` returned by one `RefStore::read_latest()` that reads pointer + map under `refstore.lock` (shared) and never errors on `Dangling`; `status` renders `Dangling` as `snapshot_id: null, snapshot_missing: "<id>"`. Delete `load_latest`/`migrate_legacy_latest`/`RefMap::load` (or wire migration into `read_latest` if legacy stores still matter). Decide and document whether drill moves the pointer and whether a zero-match find moves it. Files: `refs/refs_store.rs`, `commands/status.rs`, tests. Size S. Behavior fix (status) splits cleanly from the dead-code removal.
- Overlap: upstream #537; open PRs #538, #576, #588, #500 all patch `status.rs` only; none removes the dead migration or defines pointer semantics. Perf: none.
- Prior art: issue #537 (open); PRs #538, #576, #588 (open, status-only fix), #500 (open, optimistic re-read under retention churn), #271 (merged, stale latest-pointer doc). Searches: issues/PRs `"latest_snapshot_id"`, `"status SNAPSHOT_NOT_FOUND"`, `"status snapshot deleted"`, `"last_refmap migration"` (none); fork `gshost1/agent-desktop` PRs `snapshot|status` (none); `git log upstream/main --grep=537 / "latest snapshot"` and `-S migrate_legacy_latest` (no fix, only the 1291a9cd origin). Not DROPPED: not fixed on upstream/main.

### REFS-2: `unique_at_capture: bool` collapses four capture verdicts into one; timeouts surface as AMBIGUOUS_TARGET
- Pieces:
  - `refs/ref_entry_identity.rs:15-18` (the persisted bool).
  - Writers: `refs/ref_alloc.rs:295,307` (snapshot/drill), `locator/live_locator/evaluate.rs:112`, `materialize.rs:30`, `hydrate.rs:36-45` (find).
  - Proof: `refs/ref_capture_proof.rs:33-38` (`whole_window = Window|Application && is_exhaustive()`), `:103-119` (`proves_node`: false when not whole-window, when count != 1, or when any node left a recorded field unread).
  - Capture: `snapshot/window_capture.rs:46-95` (`identity_unproven: Option<String>`), `snapshot/mod.rs:20-24,68` (`complete`, `identity_unproven` live only in the response, never in the refmap).
  - Reader: `refs/resolve_saved_ref.rs:96-99` -> `refs/resolve_errors.rs:27-44` (`identity_not_unique`, `AMBIGUOUS_TARGET`, `retryable: false`).
- States and transitions today:
  ```
  capture verdict (real)          persisted            later process sees
  Proven (whole, exhaustive, 1)   true                 resolves
  Twin (count >= 2)               false   --------\
  CaptureIncomplete (timeout)     false   ---------+--> AMBIGUOUS_TARGET identity_not_unique, retryable:false
  FieldUnread (Unknown evidence)  false   --------/
  ```
  The response carries `complete` and `identity_unproven` (CaptureIncomplete) but the refmap does not, so a second process cannot tell the cases apart.
- Illegal / unhandled transitions:
  1. **A capture that ran out of time yields refs that later fail as "ambiguous, not retryable".** Path: `snapshot::run_with_context` (`snapshot/mod.rs:167-187`) -> `capture_window` whole-window attempt capped at 3/4 budget (`window_capture.rs:56`) fails -> shallow walk -> `identity_unproven: Some` -> allocation with a non-whole-window proof -> every tokenless entry `unique_at_capture: false` -> `click @s:eN` -> `load_ref_entry` -> `resolve_saved_ref` -> `observe_saved_ref:97-98` -> `AMBIGUOUS_TARGET`. On Windows/Linux (and macOS elements without an address) a default 3 s snapshot of a large window (doc comment in `refs/ref_resolve_deadline.rs:14-16` cites Task Manager at 2.6 s for one full search) hits this. The agent gets "ambiguous" and `retryable: false`; the right recovery is "retake with larger --timeout-ms" (TIMEOUT-class). docs/json-output.md:183 admits the conflation in prose. CONFIRMED (traced end to end).
     Test sketch: with `snapshot/tokenless_capture_support.rs` (stub whose whole-window read is cut short) take a snapshot, assert response `identity_unproven: true`, then resolve one of its refs through `commands::helpers::execute_ref_action_result_with_context`; assert the error is not `AMBIGUOUS_TARGET` (want `TIMEOUT` kind `identity_unproven` or a distinct `details.kind: "capture_incomplete"`). Fails on main (gets `identity_not_unique`).
  2. Same for `FieldUnread`: a single node whose name read returned Unknown during capture (`unread_bits`, `ref_capture_proof.rs:182-201`) makes every entry recording that field unproven forever. Reported as a twin. CONFIRMED (code).
- Hidden cost: the agent's natural reaction to AMBIGUOUS (pick another control, or headed coordinate click per the suggestion) is the wrong lever for CaptureIncomplete; retaking with a larger timeout fixes it. Not measured.
- Impact class: **B** (wrong code and wrong recovery).
- Proposed explicit form: `enum CaptureVerdict { Proven, Twin { count }, Incomplete, Unread }` stored in `RefEntryIdentity` as `capture: CaptureVerdict` with a serde shim that reads the old `unique_at_capture: true|absent` as `Proven|Incomplete` (old maps lose nothing they had). `CaptureProof::verdict(entry, index) -> CaptureVerdict` replaces `proves_node -> bool`. `observe_saved_ref` maps `Twin -> AMBIGUOUS_TARGET identity_not_unique`, `Incomplete|Unread -> TIMEOUT kind identity_unproven, retryable:false, suggestion "retake with --timeout-ms"`. Files: `refs/ref_entry_identity.rs`, `refs/ref_capture_proof.rs`, `refs/ref_alloc.rs`, `locator/live_locator/{evaluate,materialize,hydrate}.rs`, `refs/resolve_errors.rs`, `refs/resolve_saved_ref.rs`, docs/json-output.md. Size M. Refactor (enum) and behavior (error mapping) can land as two commits; the error change is the user-visible one.
- Overlap: none in the register. Envelope change (new kind) needs the version-bump contract in `docs/solutions/best-practices/envelope-version-bump-contract-2026-05-13.md`.
- Prior art: none found that addresses it; #442 (merged) introduced the capture rule and the bool. Searches: issues/PRs `"identity_not_unique"`, `"unique_at_capture"`, `"AMBIGUOUS_TARGET timeout snapshot"`; fork PRs `ref|resolve` (none); `git log upstream/main --grep=identity_not_unique` (only #442, #480, #503, no fix).

### REFS-3: A snapshot id is mutable: drill-down rewrites the map other processes hold refs from
- Pieces:
  - `snapshot/snapshot_ref.rs:65-73` (unlocked load of the root entry), `:110-120` (`update_existing_snapshot` with `remove_by_root_ref` + re-allocate), `:136-144` (response `complete: true`, `ref_count: refmap.len()` = whole map, not the drilled refs).
  - `refs/refs_store.rs:70-87` (lock, compare root entry for equality, rewrite in place).
  - `refs/mod.rs:112-139` (`remove_by_root_ref`: owned refs, same-capture descendants, transitively), `:207-214` (`same_capture_descendant`).
  - Readers: `commands/helpers.rs:325-360` (`load_ref_entry`; missing key -> `AppError::stale_ref`, `:347-358`), `snapshot_ref.rs:70-72`, `src/visual_debug/capture.rs:47,75`.
  - Trace copy: `trace/trace_artifact_budget.rs:99-118` (write-if-absent), `refs/refs_store_prune.rs:60` (#3 on main).
- States and transitions today:
  ```
  per ref key in snapshot S:
  Live(entry) --drill of an ancestor or of itself-as-owner (any process)--> Removed   (no tombstone)
  Removed --load_ref_entry--> STALE_REF "not found in current RefMap" (no details.kind)
  root R  --concurrent drill of an ancestor of R--> Removed --> drill(R) under lock: STALE_REF (refs_store.rs:80-81)
  drill(R) by P1 commits refs {e20..e25}; drill(R) by P2 commits --> P1's refs Removed, P2's {e26..}
  ```
- Illegal / unhandled transitions:
  1. **A live, unchanged element's ref turns STALE_REF because another process drilled.** Multi-agent mode shares snapshots by session (CONCEPTS.md:233 "snapshots remain shared by session"). Agent A: `snapshot` returns `@s1:e3` (container) and `@s1:e7` (button inside it). Agent B: `snapshot --root @s1:e3` -> `remove_by_root_ref("@e3")` removes `@e7` as a same-capture descendant (`refs/mod.rs:120`, path prefix) and allocates `@e12` for the same button. Agent A: `click @s1:e7` -> `load_ref_entry` -> key missing -> `STALE_REF` with "Re-run a snapshot". CONFIRMED (traced end to end).
     Test sketch: `snapshot::run(&adapter, &opts, None, None)` with a stub adapter returning window > group > button (existing `snapshot_ref_tests.rs` fixtures), record the button's token; `snapshot_ref::run_from_ref(&adapter, &opts, "<group token>", None)`; then `load_ref_entry("<button token>", &CommandContext::default())`. Assert either the entry still loads or the error carries `details.kind: "ref_superseded"` naming the drill root. Fails on main (plain STALE_REF, no kind).
  2. **Two drills of the same root clobber each other.** P1 and P2 both load `@e3` unlocked, both capture, P1 commits, P2's equality check passes (the root entry survives a drill of itself) and removes P1's freshly returned refs. P1's agent gets STALE_REF on refs printed milliseconds earlier. CONFIRMED (code; needs two processes to observe).
  3. A concurrent drill of an ancestor removes the root itself; the second drill fails STALE_REF at `refs_store.rs:80-81` after paying for a full capture. Correct code, misleading recovery ("re-run a snapshot") when a narrower retry with the new root ref would do. CONFIRMED (code).
  4. Full-artifacts session: drill skips the trace copy (exists), `end_session` then deletes the dir (#3, main). CONFIRMED by upstream; not re-reported.
  5. Classification side-note: `ref_token.rs:47-52` accepts a non-canonical number (`@s1:e07`) and builds `@e07`, which never matches the canonical key `@e7`, so a malformed token gets STALE_REF instead of INVALID_ARGS. D.
- Hidden cost: a drill that loses the race paid a full whole-window capture (depth 50) before the lock check.
- Impact class: **B** (wrong error and recovery across processes; no wrong action, because resolution is identity-based and the removed key cannot resolve).
- Proposed explicit form: make removal explicit state: `RefMap { inner, counter, superseded: HashMap<String, Superseded { root: String }> }` (bounded; cleared when the root itself is removed). `remove_by_root_ref` moves keys into `superseded`; `load_ref_entry` returns `STALE_REF` `details.kind: "ref_superseded", details.drill_root: "@s1:e3"` with a suggestion to use the drill output. Alternative (larger, cleaner): copy-on-write drill that writes a child snapshot id and never mutates a published map, which also removes #3's stale-copy problem and the response `ref_count` ambiguity. Files: `refs/mod.rs`, `refs/refs_store.rs`, `snapshot/snapshot_ref.rs`, `commands/helpers.rs`, serde tests. Size M (tombstones) / L (copy-on-write). Classification fix separable from the storage change.
- Overlap: upstream #3, SM1 I3 ("contents are NOT immutable"), SM5. Perf: none.
- Prior art: none found for the cross-process invalidation. #20 (merged) introduced drill-down; #340 (merged) documents it. Searches: issues/PRs `"drill-down stale ref"`, `"snapshot --root STALE_REF"`, `"remove_by_root_ref"`; fork PRs `drill` (none); `git log upstream/main --grep=drill` (feature commits only); `upstream/spike/perf-experiments` head 2076f717 "only discard refmaps the trace actually copied" is #3, not this.

### REFS-4: The (code, complete, retryable) triple on resolve errors is read differently by each consumer
- Pieces:
  - Producers: core `refs/resolve_errors.rs:64-89` (`saved_ref_ambiguous`, `saved_ref_absent`: complete:true, retryable:false), `refs/resolve_saved_ref.rs:160-164` (`confirm_landed` NoMatch -> `absent`); Linux `crates/linux/src/tree/resolve/resolve_errors.rs:18-34` (`unsettled`: STALE_REF complete:true **retryable:true**, `resolve_no_candidate`); Windows `crates/windows/src/tree/resolve/resolve_match.rs:4-12` (same triple, `resolve_no_candidate`); macOS `crates/macos/src/tree/resolve/mod.rs:41-57,269-282` (ElementNotFound -> `resolution_complete_absence` complete:true, retryable = `cause.permits_retry_by_default()`, normally true).
  - Consumers: ref-action pre-lease poll `ref_action/ref_action_poll.rs:103-119` (StaleRef + complete:true is terminal, retryable ignored); dispatch re-entry `ref_action/ref_action_wait.rs:140-144` (`is_retryable_resolution_failure`: honours retryable); `commands/wait_element.rs:84-90,107-109` (honours retryable); find hydration `locator/live_locator/hydrate.rs:8-20` (one retry if not NonRetryable); poll state `ref_action/ref_action_poll_state.rs:34-38,118-126` (`is_settled_identity` matches StaleRef|Ambiguous + complete:true).
- States and transitions today:
  ```
  error (STALE_REF, complete:true, retryable:true)   ["path landed nowhere / role refuted"]
     pre-lease poll       -> terminal
     dispatch under lease -> resolves_again -> back to poll -> terminal next round
     wait --element       -> keep polling until deadline
     find hydration       -> retry once
  error (STALE_REF, complete:true, retryable:false)  [core saved_ref_absent, incl. confirm_landed NoMatch]
     everywhere           -> terminal
  is_settled_identity(StaleRef, complete:true)       -> unreachable (poll returned before record_resolve_error)
  ```
- Illegal / unhandled transitions:
  1. **Churn between the whole-window count and the anchor descent is reported as settled absence.** `resolve_saved_ref` (`resolve_saved_ref.rs:37-46`) counts exactly one match at path P in a walk that can take seconds, then `resolve_locator_anchor(anchor at P)`. If a list reorders after the node was read but before the descent (Task Manager rows by CPU, chat/log views, file lists refreshing), the descent lands a same-role sibling: core `confirm_landed` -> `IdentityMatch::NoMatch` -> `absent(entry)` = `saved_ref_absent`, "No live element matches... over complete evidence", retryable:false. The element still exists; the evidence was one node, not a complete read. On a path that leads nowhere the platform returns its `resolve_no_candidate` (retryable:true) and the poll loop still stops on `complete:true`. CONFIRMED (code path traced end to end); race frequency SUSPECTED (needs a probe on a reordering list).
     Test sketch: `refs/resolve_saved_ref_test_support.rs::RuleAdapter` with `observed` = a tree where the target is unique and `landed = Some(node("button", "Other"))` (same role, different name). Call `resolve(&adapter, &capture(tree, "Target"))`. Assert the error is retryable (or a distinct kind such as `landing_raced`) and not `saved_ref_absent`. Fails on main. Second test: drive `ref_action_poll::execute_poll_loop` with a stub whose `resolve_locator_anchor` returns the Linux-style unsettled STALE_REF once then succeeds; assert the loop resolves. Fails on main (returns the error on attempt 1), while the same stub passes through `wait_element::execute`.
  2. `is_settled_identity`'s StaleRef arm (`ref_action_poll_state.rs:119`) can never fire, so `settled_identity()` timeout shaping (`ref_action_poll.rs:167-168`) only ever reports AMBIGUOUS. Dead state. CONFIRMED.
  3. `saw_ambiguity` / `transient_ambiguity` needs an AMBIGUOUS_TARGET that is explicitly retryable; every resolve-path ambiguous error found (core `:64-79`, Linux `:131-143`, macOS `resolve_classify.rs:20-37` with no stamp) is non-retryable or unstamped, so the flag looks unreachable from saved-ref resolution. SUSPECTED (Windows/Linux window-tie paths not traced).
  4. macOS `TokenLookup::Unavailable` (private symbols missing in this process) is reported as `retained_object_stale reason: "dead"` (`crates/macos/src/tree/resolve/retained_resolve.rs:51`): "element gone" when the process simply cannot resolve tokens. Same binary on the same OS loads the same symbols, so it only bites across an OS update or a different host process. SUSPECTED, low.
- Hidden cost: terminal-too-early saves time but costs the agent a re-snapshot (one full snapshot, roughly 0.3-3 s; estimate, not measured).
- Impact class: **B** (wrong error / wrong retry disposition; never a wrong action, since every path is pre-dispatch and `not_delivered`).
- Proposed explicit form: a typed verdict instead of JSON flags: `enum ResolveRefusal { Absent { settled: bool }, LandingRaced, Ambiguous, NotUnique(CaptureVerdict), Incomplete, WindowGone, ProcessGone }` carried on `AdapterError` (or returned by resolvers and converted at the boundary), with one `fn poll_action(&self) -> PollAction { Retry, Stop }` used by all four consumers. `confirm_landed` NoMatch after a count becomes `LandingRaced` (retry within budget; one re-count). Files: `refs/resolve_errors.rs`, `refs/resolve_saved_ref.rs`, `ref_action/ref_action_poll.rs`, `ref_action/ref_action_poll_state.rs`, `ref_action/ref_action_wait.rs`, `commands/wait_element.rs`, `locator/live_locator/hydrate.rs`, the three platform `resolve_errors`. Size M. The `confirm_landed` reclassification (S) can ship first.
- Overlap: upstream SM4/SM2 (register says no invalid transitions; the consumers disagree). A perf change to the poll loop (`ref_action_poll.rs`) touches the same lines.
- Prior art: none found. Searches: issues/PRs `"resolve_no_candidate retryable"`, `"saved_ref_absent"`, `"complete retryable STALE_REF poll"`, `"remote token unavailable"` (only unrelated dependabot hits); fork PRs `resolve` (none); `git log upstream/main -S is_settled_identity` (origin 483a31d0 only), `--grep=retryable` (no fix).

### REFS-5: Ref-store lock under an inherited deadline: complete results fail TIMEOUT, partial ones are saved (#10, wider)
- Pieces: `refs/refs_lock.rs:5,12-23` (2 s, `Deadline::after` folds in the inherited deadline; `acquire_detached` does not), `storage/file_lock.rs:102,107` (expired check runs before the first `try_lock` and again after success), `snapshot/mod.rs:28-34` (`save_refmap`: complete -> attached, partial -> detached), `commands/find_live.rs:174-179` (attached), `refs/refs_store.rs:70-87` (drill, attached), `commands/wait_selector.rs:207` (timeout path detached) vs `:301` (match path via `save_refmap`), inherited deadline setters `src/batch/execution.rs:23,100` only (`src/main.rs:146-148` passes `None`).
- States and transitions today:
  ```
  result built   x  inherited deadline   -> lock budget
  complete          live                    min(2s, remaining)  -> saved
  complete          expired                 0 -> TIMEOUT before try_lock, even uncontended   [illegal]
  partial           any                     2s detached -> saved
  ```
- Illegal / unhandled transitions: in `batch`, a snapshot/find/drill/wait-for item whose read completes just inside the batch budget (allocation and serialization are CPU work after the walk) returns `TIMEOUT` and persists nothing, while the same item had it come back incomplete would be saved with `ok: true`. For a drill the capture is thrown away. CONFIRMED (code traced; batch-only).
  Test sketch: in a core test, install an expired inherited deadline (`wait::deadline` scope helper used by batch) and call `SnapshotResult{complete: true, ..}.save_refmap(&store)`; assert `Ok`. Fails on main with `TIMEOUT`. Same with `find_live::persist_refmap`.
- Hidden cost: none extra; it wastes the completed capture.
- Impact class: **B**.
- Proposed explicit form: persist is post-result recovery work; give `RefStore` one entry `persist(result, Persist::AfterResult)` that always uses the detached 2 s lock for any result already built, and keep the attached lock only for read-modify-write that must respect the caller (none today). Files: `snapshot/mod.rs`, `refs/refs_store.rs`, `commands/find_live.rs`, `snapshot/snapshot_ref.rs`. Size S. Upstream marks this "needs a policy decision"; the asymmetry (partial saved, complete refused) is the argument for detached.
- Overlap: upstream #10. Perf: none.
- Prior art: upstream register #10 (no issue/PR filed); related merged #372 (detached save for partial snapshots), #164 (wait_timeout envelope under inherited deadline), closed issue #154 (batch waits returned lock_timeout). Searches: issues/PRs `"ref store lock timeout batch"`, `"save_refmap deadline"`; fork PRs `snapshot` (none); `git log upstream/main -S save_new_snapshot_detached` (53e072dc #372, 07913a7c). Not fixed on upstream/main or the laf branch.

### REFS-6: Resolution and capture read the window more than once; deadlines restart mid-command
- Pieces: `refs/resolve_saved_ref.rs:28-47` (one resolve = `recorded_window` window lookup (`:172-208`, plus `list_windows` when no window id) + whole-window walk depth 50 (`:100-108`) + platform anchor descent, which re-opens the window (`crates/linux/src/tree/resolve/resolve_anchor.rs:19`, `crates/windows/src/tree/resolve/resolve_anchor.rs` `resolve_window_root`, macOS `candidate_roots`) + `confirm_landed` node read (`:139-165`)); `:58-77` land fast path with full-resolve fallback; `ref_action/ref_action_poll.rs:43` (land per poll) and `ref_action/mod.rs:80` (full resolve under lease), `:119` (third resolve after a scroll that returns no handle); `commands/wait_element.rs:35` (full resolve every 100 ms poll, `refs/ref_resolve_deadline.rs:10`); `commands/get.rs:28-29`, `commands/is_check.rs:31,42` (resolve with `Deadline::standard()` 5 s, then a fresh 5 s deadline for the read); `refs/ref_root_read.rs:27-37` (token root: window lookup + strict resolve + subtree read, then, if the subtree has tokenless refs, a second window lookup + whole-window walk); `snapshot/window_capture.rs:55-67` (non-token adapters: whole-window attempt at 3/4 budget, on failure a second shallow walk with the remaining quarter).
- States: implicit "attempt ladders" in each function (land -> full; token subtree -> whole window; whole window -> shallow).
- Illegal / unhandled transitions: none wrong; the cost is the finding. `get role` / `get title` pay a full whole-window resolve and then return the stored snapshot values (`get.rs:31-32`), so the read proves liveness only.
- Hidden cost (estimates unless noted; whole-window walk W = 50-300 ms typical, 2.6 s Task Manager per the in-code note in `ref_resolve_deadline.rs:14-16`):
  - Ref action, common case: 1 land (window gate + path + node) + 1 full resolve (2 window lookups + W + path + node) + stable preflight (>= 3 samples over >= 34 ms). One W, two to three window lookups. Landing failure during polling: +1 W per poll iteration that fails.
  - `wait --element`: one W per 100 ms poll; a 5 s wait on a large window spends ~all of it walking.
  - `get`/`is`: up to 10 s wall time against a nominal 5 s budget (outside batch).
  - Drill/rooted find on a token root with tokenless children: subtree read wasted, then W.
  - Snapshot on Windows/Linux when the whole window does not fit 3/4 of the budget: 0.75 x budget spent and discarded, then a shallow walk; refs come back unproven (REFS-2).
- Impact class: **C**.
- Proposed explicit form: `enum ResolveTier { Land, Count, CountAfterLand }` with one ladder function shared by poll, dispatch and wait, so wait-element can poll on `Land` and count once at the end (a predicate wait never acts, which is the documented condition for the land handle); pass the recorded `WindowInfo` from `recorded_window` into the platform anchor call to drop the second window lookup (adapter trait change, platform crates touched); one `Deadline` per command in `get`/`is`. Files: `refs/resolve_saved_ref.rs`, `refs/ref_resolve_deadline.rs`, `commands/wait_element.rs`, `commands/get.rs`, `commands/is_check.rs`, `adapter/observation.rs`, three platform `resolve_anchor`s. Size M. Measure first (`a-cited-measurement-must-match-its-capture.md`).
- Overlap: any perf change to `resolve_saved_ref.rs`, `ref_action/mod.rs`, `wait_element.rs`, `window_capture.rs`. No `perf/` branches exist in `origin` (checked `git branch -r`); `upstream/spike/perf-experiments` and `upstream/feat/cursors-and-performance` do not touch these files (diff --stat empty).
- Prior art: none found. Searches: issues/PRs `"wait element resolve cost"`, `"whole-window read double"` (only unrelated Windows/sessions PRs); fork PRs `resolve` (none); perf branches diffed as above.

### REFS-7: Count-based retention (128 -> 96 by dir mtime) in a namespace shared by every agent
- Pieces: `refs/refs_store.rs:16-18`, `refs/refs_store_prune.rs:72-120` (mtime sort `:108`, only the just-saved id protected `:114`), callers creating snapshots: snapshot, every `find` (`find_live.rs:64-65`, including zero matches and rooted finds), wait-for match and timeout (`wait_selector.rs:207,301`). DirOnly orphans (crash before rename) count toward the cap and are only removed on eviction (`:95-97` sweeps tmp files of the latest dir only).
- States and transitions: `Retained(id, mtime)` -> `Evicted` when 129 dirs exist and id is among the 33 oldest by mtime; drill refreshes mtime (rename into the dir), read-only use does not.
- Illegal / unhandled transitions: in the global (no-session) namespace or a shared multi-agent session, an agent polling with `find` creates one snapshot per call; 33+ calls by anyone evict the snapshot another agent is mid-way through acting on. Its next command gets `SNAPSHOT_NOT_FOUND` whose suggestion blames session scoping (`response/output_tests.rs:70-79` pins that text), not retention. CONFIRMED (code); frequency SUSPECTED.
  Test sketch: `HomeGuard`, save snapshot A, then 128 `save_new_snapshot` calls (existing `refs_store_pruning_tests.rs` pattern), then `load_ref_entry("@A:e1")`; assert the error says retention evicted it (e.g. `details.kind: "snapshot_evicted"`), which needs an eviction tombstone. Fails on main.
- Hidden cost: prune pass (read_dir + stat of ~129 dirs + 33 `remove_dir_all`) once every 33 saves; amortized small (estimate < 5 ms).
- Impact class: **B** (wrong recovery text), low.
- Proposed explicit form: keep a bounded `evicted` list (id + time) next to `latest_snapshot_id`, written under the same lock by prune; `load_snapshot` turns a miss on an evicted id into `SNAPSHOT_NOT_FOUND kind: "evicted"` with a retention-specific suggestion. Optionally stop find from persisting an empty map. Files: `refs/refs_store.rs`, `refs/refs_store_prune.rs`, `commands/find_live.rs`. Size S.
- Overlap: upstream PR #389 (opt-in TTL) adds a second eviction trigger and would make this more frequent.
- Prior art: PR #389 (open, TTL eviction); #500 (open, status under retention churn). Searches: issues/PRs `"prune snapshots 128"`, `"SNAPSHOT_NOT_FOUND pruned"`; fork PRs `prune` (none); `git log upstream/main --grep=prune` (no change to retention semantics).

### REFS-8: Test-only RefStore surface that encodes transitions production never takes
- Pieces: `refs/refs_store.rs:66-68` (`save_snapshot` explicit id: no pointer, no prune), `:150-152` (`set_latest`), `:104-114,263-283` (`load_latest`, legacy migration), `:158-184` (`retained_object_tokens`: fails the whole call on one DirOnly dir (SNAPSHOT_NOT_FOUND) or > 128 candidates (INVALID_ARGS)), `refs/mod.rs:185-191` (`RefMap::load`, pub). No non-test caller in `crates/` or `src/` (grep).
- States: these functions allow a snapshot dir without a pointer update and without prune (`save_snapshot`), a pointer to any id (`set_latest`), and a store-wide token census, none of which production reaches.
- Illegal / unhandled transitions: none reachable; tests built on them (`refs_store_pruning_tests.rs:53,86-88`, `refs_store_retained_tests.rs`) exercise states production cannot create, so they can pass while the real path regresses (`docs/solutions/best-practices/a-test-that-cannot-fail-is-not-coverage.md`).
- Hidden cost: none.
- Impact class: **D**.
- Proposed explicit form: delete or make `#[cfg(test)]`; rewrite the dependent tests through `save_new_snapshot`. Size S.
- Overlap: REFS-1 (legacy path).

### REFS-9: Loose field combinations inside RefEntry
- Pieces: `refs/ref_scope.rs:4-11` (`root_ref: Option<String>`, `path_is_absolute: bool`); writers keep them in lockstep (`refs/ref_alloc.rs:294`, `locator/live_locator/materialize.rs:182-186`); readers branch on both (`crates/linux/src/tree/resolve/mod.rs:58-60`, `crates/macos/src/tree/resolve/mod.rs:294-301`, Windows `window_rooted`, `refs/mod.rs:211`). `refs/ref_entry_identity.rs:6` `retained_object: Option<String>` is either an element token (`axtok1:` prefix, `refs/ref_identity.rs:13-25`) or a "retained location", discriminated by string prefix. `RefSource::is_process_owned` (`refs/ref_source.rs:24-26`) is `window_id == None && window_title == Some("")`, a sentinel. `refs/refs_validate.rs` checks none of these combinations.
- States: `(None,false)` window capture, `(Some,true)` drill/find-rooted, `(Some,false)` legacy-only (older maps), `(None,true)` unreachable from current writers. Token vs location vs none. Process-owned vs window-owned by sentinel.
- Illegal / unhandled transitions: a legacy `(Some,false)` entry is "not window rooted" on every platform, so its anchor fails (`resolve_anchor.rs` Windows returns `stale_ref_error` immediately), and core's full resolve still works because it builds a fresh anchor. No wrong outcome found. CONFIRMED no reachable bad state from current writers.
- Hidden cost: none.
- Impact class: **D**.
- Proposed explicit form: `enum RefPathScope { Window(RefPath), Drill { root: String, path: RefPath } }` with serde compat (`root_ref` + `path_is_absolute` read into it, `(Some,false)` mapped to `Drill` with a relative-path marker or refused at load); `enum RetainedObject { Token(String), Location(String) }`; `enum RefOwner { Window { id, title }, Process }`. Size M (touches every resolver), pure refactor.
- Overlap: REFS-2 (same struct).

---

## Coverage notes
- Looked at: `refs/*` (store, prune, lock, mod, entry types, capture proof, alloc, resolve, deadline, errors, token, root read), `snapshot/{mod,snapshot_ref,window_capture}.rs`, `commands/{snapshot,find,find_live,get,is_check,status,wait_element,wait_selector,helpers}.rs`, `locator/live_locator/{resolve,hydrate,evaluate(part),selection_completeness,observed_tree(part)}.rs`, `ref_action/{ref_action_poll,ref_action_poll_state(part),ref_action_wait(part),mod(part)}.rs`, `storage/file_lock.rs` (part), `session/mod.rs` end_session, Linux `tree/resolve/{mod,resolve_anchor,resolve_errors}.rs`, macOS `tree/resolve/{mod,resolve_search,resolve_classify,retained,retained_resolve}.rs`, Windows `tree/resolve/{resolve_anchor,resolve_match,resolve_pacing,mod(part)}.rs`.
- Not traced: FFI ref paths (`crates/ffi`), macOS `resolve_roots.rs` window scoping, Linux `resolve_gate.rs`, Windows `resolve_descent.rs` budgets, tree/ completeness types beyond `ObservedTree`.
- Verified non-issues: refmap reads are atomic (temp + rename), so a reader never sees a torn map during a drill; `update_existing_snapshot` re-checks the root under the lock; prune never evicts the just-written latest; snapshot ids are not reused.
