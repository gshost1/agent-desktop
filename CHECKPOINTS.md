# Laptop checkpoints (while the Mac control thread is offline)

Newest last. The laptop lead appends here when a send to the control thread fails.

## Needs George

- Laptop `/tmp` cleanup (George approved): checks passed (236 `/tmp/trace-input-*` dirs, 1.2 GB, all last written 2026-11-07 22:39 or earlier; `/tmp/frie-mini-sanitized` 563 MB, last written 2026-11-03 22:27; `lsof` and every process cwd/root: no user). The lead's `rm -rf` was **denied by the session's permission layer**, so nothing was deleted. George, on the laptop: `df -h /tmp; rm -rf /tmp/trace-input-* /tmp/frie-mini-sanitized; df -h /tmp`

## Log

- 2026-11-10 ~01:15Z: control going offline ~01:30Z. Laptop lead acting orchestrator. Open: #81 (LIFECYCLE-4) green and CLEAN, landing by agent A; #83 (STATE-10) CI running; Mac-check drafts #80 #86 #58 #34 (#18 after #34) stacking in MAC-CHECKS.md; web session #84 green.
- 2026-11-10 ~01:35Z: send to control failed ("MacBook Air (36) did not answer"); Mac offline. CLI-time agent started (Opus 5.5 high, own worktree), all five steps, measure first. Slots were full (A, B, C, #58, #34, web), so it starts with step 3 (code reading: every sleep/poll/floor a successful command pays) and runs every build and timing loop under the shared gate lock (no extra concurrent full build); it takes a full slot when the next agent finishes. Interim tables: `.compound-engineering/perf/cli-time.md` on the laptop; the step-1 table will be copied here. #41 and ledger rows below ~#8 queue behind it.
- 2026-11-10: #58 (ACTION-12) up to date, CI 13/13 and gates GREEN on `d57dadfa`, code-review high applied (3 fixed with invert-verified tests); Mac check added to MAC-CHECKS.md. Waiting for the Mac.
- 2026-11-10 ~02:00Z: CLI-time agent's step 3 (code reading) is done. Copied below for the macOS CLI-latency worker, so the two don't duplicate core fixes. The laptop agent owns core, src/, web and Linux. The Mac worker owns crates/macos/**. Paying on every success, biggest first: the raised-surface walk after every click-like action (`ref_action/settled_surface.rs`, up to 1 s, a walk rather than a sleep); Linux web-engine readiness (≥400 ms plus 3 walks per snapshot/find); Linux window ops (120 ms); Linux focus (50 ms); 4 fsyncs per snapshot. Not yet measured: step 1 (fixed cost per invocation) is waiting for the release build behind the gate lock.

### CLI-time step 3: fixed sleeps, polls and floors a successful command pays

"Pays on success" means the wait runs even when the first observation already shows the outcome.
"Quantizes" means the wait runs only while the app has not yet reached the state. The interval then adds 0 to one full interval after the real change.

#### Core and binary (all platforms)

| file:line | duration | who pays | pays on success? | event-driven / shorter? |
|---|---|---|---|---|
| `crates/core/src/actions/post_action.rs:59-74` | 50 ms poll, capped 400 ms | stateful ref actions (set-value, type, toggle, check, uncheck, select, expand, collapse) via `execute_verified_action` | only while the postcondition is false. Quantizes: a commit at +5 ms costs 50 ms | a 10–16 ms poll keeps the same 400 ms cap. Average saving about 20 ms whenever the first read-back lags |
| `crates/core/src/ref_action/mod.rs:208-230` + `actionability/stability_sampler.rs:4-5` | 17 ms samples, at least 34 ms span and 3 samples | ref actions whose live bounds hash differs from the snapshot's, or that have no bounds hash | no when bounds are unchanged (permissive pass). Otherwise at least 34 ms plus 3 bounds reads | Playwright uses 2 animation frames. 34 ms is already about 2 frames, so keep it |
| `crates/core/src/ref_action/ref_action_poll.rs:19,90-95` | 16 ms (stable) / 100 ms (other) | ref actions whose preflight fails retryably | no (only on a failed preflight) | 100 ms quantizes the ref auto-wait. 25–50 ms would be fairer, but it costs extra full resolves |
| `crates/core/src/ref_action/ref_action_wait.rs:137` / `refs/ref_resolve_deadline.rs:10` | 100 ms | resolve retry after a transient STALE/AMBIGUOUS | no | same as above |
| `crates/core/src/ref_action/settled_surface.rs:12,26` | up to 1000 ms (a walk, not a sleep) | **every successful click, double-click, right-click, triple-click, expand, select and press** (`may_raise_surface`) | **yes**: `list_raised_surfaces` runs after every such action | measure per platform (step 5). It could be skipped when the action kind cannot open a surface, or bounded to the target's process top-level children |
| `crates/core/src/commands/wait_scheduler.rs:5` | 100 ms | `wait --element/--text/...` and `--wait-for` | quantizes: up to 100 ms after the condition holds | event-driven through AT-SPI/AX signals (agent C owns `wait/signals.rs`, `wait_event.rs`) |
| `crates/core/src/commands/wait_element.rs:98` | 100 ms | `wait --element` | quantizes | same |
| `crates/core/src/commands/wait.rs:190` | 100 ms | `wait` condition loop | quantizes | same |
| `crates/core/src/storage/file_lock.rs:122` | 10 ms | any file lock under contention (refstore, session) | no (only under contention) | a blocking `flock` with a timeout thread would be exact. Low value |
| `crates/core/src/session/process_lease_guard.rs:31` | 1 ms spin | in-process lease contention | no | fine |
| `crates/core/src/tree/renderer_accessibility.rs:13-14` | 25→400 ms backoff | first observation of a Chromium/Electron renderer whose tree is not yet published | only on first activation | fine |
| `crates/core/src/locator/live_locator/resolve.rs:189,218` | 10 ms | locator re-read on a transiently incomplete tree | no | fine |
| `crates/core/src/commands/close_app.rs:11` | 25 ms | `close-app` confirm loop | quantizes until the process exits | a pidfd/exit wait would be event-driven |
| `crates/core/src/commands/type_text_interval_verify.rs:17-18` | 25 ms poll, 500 ms settle | `type --interval` | quantizes | fine (opt-in) |
| `crates/core/src/cursor_overlay/caret_follow.rs:13-16` | 60–240 ms | headed cursor overlay only | headed only | out of scope (headed UX) |
| `crates/core/src/ref_action/presentation.rs` | up to 900 ms (cursor arrival) | headed only | headed only | out of scope |
| `crates/core/src/commands/pointer_action.rs:219-230,301` | 16 / 100 ms | mouse-click/hover/drag point retry | no | **PR #58 owns this file**. Do not edit |
| `crates/core/src/refs/refs_lock.rs:9` | 2000 ms timeout | refstore lock | no (timeout only) | fine |
| `crates/core/src/storage/private_file.rs:127,244` | 2× fsync per write (file + parent dir) | every snapshot (refmap.json + latest_snapshot_id = 4 fsyncs plus a dir create), session writes, trace | **yes** | measured in step 2 |

#### Linux adapter

| file:line | duration | who pays | pays on success? | event-driven / shorter? |
|---|---|---|---|---|
| `crates/linux/src/actions/focus.rs:80-86` | **50 ms unconditional** after focus is first seen | every successful `focus` (and any path through `focus_read_back`) | **yes, always 50 ms** (corroborating second read) | candidate: corroborate with an immediate second read, or a 10 ms gap |
| `crates/linux/src/actions/click.rs:25-48` `poll_until` | 50 ms step, 300 ms / 1500 ms (popup) budget | right-click (observation), popup openers via `popup_verify.rs:168`, toggle (`toggle_state.rs`), select (`select/*`), disclosure (`disclosure.rs`) | quantizes: first read is immediate, then 50 ms steps | 10–16 ms step. AT-SPI `StateChanged` / `ChildrenChanged` signals would be event-driven |
| `crates/linux/src/tree/observe/readiness.rs:7-9` | **200 ms × 2 gaps (≥400 ms) + 3 full walks** | **every snapshot/find of a web-engine window over AT-SPI** (Chrome, Firefox, Electron) | **yes**: needs 3 equal node counts 200 ms apart | candidate: a shorter gap (50 ms) with the same 3-read rule, or `ChildrenChanged` quiet-period detection. Must be re-verified against the L2 VS Code / Chrome stabilisation series |
| `crates/linux/src/system/window/window_op/mod.rs:44` + `window_op_hold.rs:16` | **120 ms unconditional** | move-window, resize-window, maximize (confirm the WM did not re-place) | **yes** | WM `ConfigureNotify` would be event-driven. Measure on openbox and mutter first |
| `crates/linux/src/system/window/window_op/window_op_verdict.rs:14,19` | 25 ms poll, 250 ms settle | window ops | quantizes | fine |
| `crates/linux/src/input/mouse/pointer_plan.rs:10-11` | 10 ms hold + 20 ms gap | physical clicks (headed) | yes (physical only) | needed for toolkit double-click detection. Keep |
| `crates/linux/src/input/keyboard/text_plan.rs:15-16` | 50 ms per remapped keysym + 50 ms restore | physical typing of characters missing from the keymap | yes (physical, unusual chars only) | keep (X server keymap propagation) |
| `crates/linux/src/input/x11/verify.rs:14-15` | 10 ms poll, 250 ms cap | physical input leftovers | quantizes | fine |
| `crates/linux/src/actions/scroll/scroll_plan.rs:11,14` | 100 ms poll, 2 s | scroll observation | quantizes | 25 ms poll |
| `crates/linux/src/system/menu/menu_wait.rs:12` | 50 ms | menu waits | quantizes | **#18 owns the wait/menu files**. Check before editing |
| `crates/linux/src/system/launch/*` | 50 ms polls | launch | quantizes | **agent A owns these** |
| notifications/* | 50–100 ms polls, 2.5–6 s settles | notification commands | partly | **PR #34 owns these** |

- 2026-11-10: /tmp cleanup verified safe but not done: the session's permission layer denied the `rm -rf`. /tmp is at 5.7 of 7.5 GB (76%). The command for George is under Needs George.
- 2026-11-10: CLI-time step 1 is done. The fixed cost per invocation on Linux is 2–4 ms (release build, no app): process start is not where CLI time goes. Linux permission preflight is about 1.5 ms. Time goes into per-action work: step 3's raised-surface walk, readiness gaps and settles. Steps 2, 4 and 5 are next.

### CLI-time step 1: fixed cost per invocation (release build, no app)

Base `644dc78e`, release `cargo build --release -p agent-desktop` (7.97 MB). `AGENT_DESKTOP_HOME=/tmp/clitime-home` (empty). Wall time is fork+exec to exit, from Python `subprocess.run`. Method: one discarded warm-up, then seven runs: min, median, max. The run held `gates.serial.lock`. Load before: 2.44; after: 1.89. perf is **unavailable** (`perf_event_paranoid=4`, no sudo), so attribution comes from `strace -c -f` / `strace -f -tt`.

| command | min ms | median ms | max ms | exit | result |
|---|---:|---:|---:|---:|---|
| `/bin/true` (spawn floor) | 0.36 | 0.49 | 0.61 | 0 | |
| `--version` | 1.72 | 1.98 | 2.04 | 0 | |
| `version` | 1.93 | 1.97 | 2.10 | 0 | ok |
| `status` | 3.77 | 4.14 | 4.34 | 0 | ok |
| `skills get desktop` | 1.88 | 1.94 | 2.10 | 0 | ok |
| `click @e1` (bare ref) | 3.46 | 3.52 | 3.85 | 1 | INVALID_ARGS |
| `click @s00000000:e1` (missing snapshot) | 3.46 | 3.60 | 3.76 | 1 | SNAPSHOT_NOT_FOUND |
| `list-apps` | 3.70 | 3.81 | 4.01 | 0 | ok |

Breakdown (strace timeline of `click @e1`; strace inflates the absolute times by about 3x, so the shares matter more than the numbers):

| phase | share | evidence |
|---|---|---|
| exec + dynamic link (libc, libm, libgcc_s) + Rust runtime start | ~1.3 ms of the ~2 ms `version` floor | 99 syscalls for `version`, mostly mmap/brk/mprotect. No file I/O beyond ld.so |
| clap parse + state-root validation | <0.3 ms | 6 `statx` of `$HOME` and `AGENT_DESKTOP_HOME` (state root resolved 3 times, cheap) |
| session/interaction lock, refmap read, trace write, snapshot-store pruning | **0** for these commands | no `openat`/`flock` under the state root: no session is active, `click @e1` fails before any refmap read, and the missing snapshot is a single failed `openat` |
| permission preflight (Linux: session bus connect → `org.a11y.Bus.GetAddress` → AT-SPI bus connect → 2 calls, 4 tokio/zbus threads) | **~1.5 ms** | `status`, `list-apps` and both `click` cases pay it. `version`/`skills` do not |
| JSON emit | <0.1 ms | one `write(1, …)` |

Findings:
- The fixed cost is 2–4 ms, far below the 100 ms bar. Process start is not where CLI time goes.
- `click @e1` (bare ref, `INVALID_ARGS`) connects to two D-Bus buses before it rejects the argument. Validating the ref token before the permission preflight saves about 1.5 ms on that path only, an error path. Low value.

