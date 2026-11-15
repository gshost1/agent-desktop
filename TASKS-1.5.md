# agent-desktop 1.5: laptop lead (core, Linux, Windows, persistent mode)

Ship target Mon 2026-11-16. Untracked in the main clone; copied to docs/state-machine-ledger at checkpoints.
Worker rules: `.agents/lead15/RULES.md`. Build slots: `.agents/lead15/cargo-slot.sh` (2 concurrent builds, jobs 3).
Bench data (2026-11-11, Linux VMs): `~/.t3/scratch/2026-11-11-you-own-a-linux-run-9e607cbc/linux-bench/results/claude-sonnet-5-5/combined/results-native.jsonl`.

## Items

- [ ] A. Linux losses: loss table + causes + tool-side fixes (worker `linux-losses`)
- [~] B. Fixed token overhead: #167 open (review running)
- [ ] C. `click -w` burns 30 s when outcome undecidable (worker `wait-decidable`)
- [ ] D. Change-only reads: `snapshot --since` + ref-action opt-in (worker `since-diff`)
- [ ] E. Persistent-process daemon: MOVED to the VPS lead (control, ~16:35); branch feat/daemon-mode from bd376a213 is theirs. No laptop daemon worker, no pushes to that branch.
- [ ] F. Queued rows
  - [x] CHANGELOG-FRAGMENTS-1 (#142 merged)
  - [ ] TEST-ISOLATE-SESSION-1 (`hygiene`)
  - [ ] LOCK-LABEL-2 (`hygiene`; #41 merged, unblocked)
  - [ ] STRICT-TRACE-CLOSE-1 (`hygiene`; #18 still draft, done on main anyway)
  - [ ] LINUX-LAUNCH-ATSPI-INTERNAL-1 (worker `linux-launch`)
  - [ ] PREFLIGHT-DISPLAY-PASSTHROUGH-1 (`linux-launch`; #124 merged)
  - [ ] SNAPSHOT-FSYNC-1 on perf/refstore-no-fsync (worker `cli-time`)
  - [ ] FIND-TEXT-SLOW-1 (`find`)
  - [ ] Windows CI flakes, wip/test/windows-ci-flakes: PR + 3 CI runs (worker `windows`)
- [~] G. Windows latency (measured: parity; see READY section) fork vs upstream on hosted runners (worker `windows`; control item 8). Workflow is control PR #154; dispatch with --ref perf/windows-competitive-latency once merged.

### Control additions (2026-11-14)
- [ ] 1. Web `ref -w` reads the page: wip/fix/web-ref-wait-surface 6cfef33e -> PR (worker `web-ref-wait`)
- [ ] 2. G1 headed hover/drag raise before non-unique refusal (worker `hover-raise`)
- [ ] 3. G3 empty snapshot from `find --first` (worker `find`)
- [ ] 4. G4 `find --first --allow-partial` partial result (worker `find`; same files as G3, so one worker, two PRs)
- [x] 5. G2 (#143 merged) CHANGELOG "350 ms" wording (worker `hygiene`, owns CHANGELOG.md)
- [ ] 6. Ledger correction + ACTION-7, ACTION-10, REFS-6, STATE-10, STATE-11, ADAPTERS-10, LIFECYCLE-4 (worker `ledger-speed`)
- [ ] 7. #18 after #34 lands: merge main, gates, Mac check into MAC-CHECKS.md (lead, waiting on #34)
- FIND-TEXT-SLOW-1 moved from `cli-time` to `find` (same locator files).

## Workers (2026-11-14 ~13:15 PST; Opus 5.5 high, own worktree .agents/worktrees/l15-<name>)
hygiene, linux-launch, wait-decidable, skills-tokens, since-diff, daemon, linux-losses, cli-time, windows,
web-ref-wait, hover-raise, find, ledger-speed. Native worktree isolation refused the repo (`.claude` is a committed
symlink to `.agents`), so the lead made the worktrees with `git worktree add --detach`.
Set `upstream` push URL to DISABLED in the main clone.


## After the OOM (control, ~15:30 PST): hard limits and queue
- OOM ~15:10 killed every worker. Limits: <= 4 laptop workers alive; all cargo through `~/bin/build-slot` (2 slots machine-wide, shared with tabref); `free -m` available >= 3000 MB before a build or worker; narrow local gates, CI is the full gate.
- Workers now open their PR and END (lead watches CI, resumes on red).
- Open PRs from the laptop: #156 hygiene #142-followup, #157 PREFLIGHT-DISPLAY-PASSTHROUGH-1, #158 TEST-ISOLATE-SESSION-1 (CI pending).
- Queue (by value):
  1. PRs for finished branches: [running] flake-focus, web-ref-wait, wait-decidable (C+C14), hover-raise (G1); [next] find G3, linux-losses C8, hygiene file-lock-deadline-label, windows ci-flakes-2.
  2. skills-tokens B (size cliff; Mac's trimmed desktop+platform 17,205 B, main 29,285 B).
  3. since-diff D, then agent-errors (stderr), then cli-time fsync.
  4. (daemon E moved to the VPS.)
  Then: C3, G4, FIND-TEXT-SLOW-1, LINUX-LAUNCH-ATSPI-INTERNAL-1, C1, STRICT-TRACE-CLOSE-1, ledger-speed rows, Windows A/B (#154), C4/C9/C12.
- Linux bench re-run waits: boxd `list` hangs from the laptop.

## A: held-out causes (from linux-losses, .agents/lead15/A-losses.md) and where they went
- Fork is really 38/40 vs 36/40 if the 2 bench audit false positives count as passes (harness `_segments()` splits `|` inside quotes).
- C1 launch 5 s bus grace -> `linux-launch` item 3. C3 window-vs-screen bounds hash, C14 `click -w` misses new dialog window -> `wait-decidable`.
- C4 refusal text (--headed), C12 disabled target waits 5 s, C9 twin-ref text, C15 stderr error line -> new worker `agent-errors`.
- C5/C6/C7/C13/C15-skill -> `skills-tokens`. C8 headed click hidden-point fallback -> `linux-losses` (fix/linux-headed-click-hidden-point-fallback).
- C10 (`--surface alert` ~1 s), C11 (LO dialog keyboard) not reproducible without LibreOffice; boxd not signed in.

## READY for control (CI green, needs second-worker review)
- 18:40 control CI rules: no main merges into open PRs unless CONFLICTING; push once; no new PRs while >=10 laptop PRs open (17 open). #166 3x proof-run loop STOPPED (it re-queued all 14 jobs per run); the PR's own CI run is the proof for now.
- GREEN: #160 (18:45), #157, #163, #165 , #174 (all merged); #170, #158, #162, #164, #175 merged.
- STATUS sent 20:10 (asked to open fix/wait-text-depth despite cap). windows worker: FFI-SESSION-FLAKE-1 + Windows --headed type (branches, no PR).
- #173 G4 (01aedc5f): first-pass --first answer kept under --allow-partial; test confirmed the bug first. Lead review MERGE.
- PARKED (branch, no PR; queue rule): perf/find-text-evidence 03ea513c (FIND-TEXT-SLOW-1: selected text queries read full evidence only at root; wait --text polls gated by a cheap probe; Linux equal within noise, win expected on macOS; body ~/.cache/agent-desktop-target/pr-body-find-perf-find-text-evidence.md). Mac check: Weather wait --text 7/7 timeouts may be max_depth 10 in wait vs 50 in find.
- perf/find-text-evidence pre-review FIX (blocking: macOS probe at logical depth skips matching polls); find worker fixing on the branch.
- PARKED: perf/linux-keyboard-one-session 34c4c4c9 (ADAPTERS-10: one XTest run per key action + layout fingerprint guard; local Xvfb ~1 ms faster; body ~/.cache/agent-desktop-target/pr-body-linux-losses-perf-linux-keyboard-one-session.md).
- ROOT CAUSE (release-blocker): every agent-desktop process registers RegisterEvent("window:activate") (also upstream) and drops it on exit right after keys; the app's at-spi2-atk bridge tears down mid NotifyListenersSync -> GTK3 segfault, Chrome too. Repro without agent-desktop 4/8; no registration 0/8. linux-losses fixing on fix/linux-xtest-key-crash, may open PR despite cap.
- #182 XTEST key crash fix (52491b1d): a `sleep` child holds the AT-SPI socket 1 s after key plans; GTK3 3/8 -> 0/12 crashes, Chrome 5/8 -> 0/10, <1 ms latency. Upstream crashes 3/8 too. Second-worker review running.
- #181 opened (fix/wait-text-depth), control approved.
- FOUND (high): GTK 3.24 GtkEntry segfaults on the first XTEST key from `--headed press` (main and branch); bench Chrome segfaulted on agent-desktop key presses where xdotool did not. -> linux-losses investigating.
- PARKED (high value): fix/wait-text-depth (+lead skill line) - wait --text snapshot depth 10 -> 50 like find --text; gnome-text-editor words past the title: TIMEOUT 2.0 s -> found in 86-89 ms; no poll slowdown. Likely explains Mac Weather 7/7 wait --text timeouts (Mac check). Body ~/.cache/agent-desktop-target/pr-body-find-fix-wait-text-depth.md.
- #178 C4+C12+C9 error texts (08aff9af; #180 closed, merged into it): GTK3/GTK4 never-enabled button 5.0 s TIMEOUT 'busy' -> 2.05-2.2 s target_disabled; 1 s-timer button still clicks. Review FIX (blocking: E2E on 3 OSes expect old 5 s TIMEOUT; `type` never gets the cut; docs/skills contradict; C4 hint vs 'never switch to headed on your own') -> fixed at caa854a6 (E2E legs on 3 OSes updated, unrun locally; type gets the cut via a default-budget flag on the request); re-review MERGE (nits only, fold into a later push if forced: type --interval-ms default budget; 'other than the default' wording; refs-and-retry sentence; menu-bar title keyboard route in skill/docs; serde default for the flag). Pending doc texts (commands-interaction.md Actionability, json-output identity_not_unique row) go into the same batch. Open: ref_action_poll.rs timeout() disabled suggestion (wait-decidable, later).
- #177 C1 Linux launch still-starting (cb80f359 incl. lead docs commit): GTK3 8 s app 5.45 s no window -> 8.75 s window; GNOME Settings 5.22 s -> 3.30 s; datetime handoff 30.14 s ACTION_FAILED -> 7.93 s window. Trade-off: a windowless never-joining process waits to --timeout. Review FIX (blocking: resident adoption can pick the wrong process / reports unobserved success; misleading still-starting text; skill contradictions) -> fixed at 357f08e8; re-review: 14/14 fixed, 1 new blocker (late new window after 2 s settle no longer reported) + 4 nits -> fixed at cb5b2a13 (5 s handoff settle; off-bus window must stay off 2 s, xlogo now 7.1 s); lead accepts (targeted fixes). linux.mdx display row to move to #157. LINUX-LAUNCH-ATSPI-INTERNAL-1: closed, no longer reproduces (3x6 commands on main 8a24365d). Left: batch `launch` still PERM_DENIED on a dead display (needs a PermissionReport field across adapters).
- NOTE: lead moved scratchpad dirs lo/item2/item3/l15ll/pwsh/... to ~/.cache/agent-desktop-target/scratch-archive/ (RAM tmpfs was 79%); linux-launch lost track of its item3 files there.
- #176 REFS-6 get/is one 5 s budget (e6637789): lead review MERGE. Ledger corrected e5bf1ef7a. wait --element land-poll not shipped (slower; contract call for George if report-from-landing is wanted). ACTION-7 -> wait-decidable after #162/#174 merge; ADAPTERS-10 -> linux-losses (started). STATE-10/11 accepted (<0.3 ms). LIFECYCLE-4 fixed by #81. ACTION-10 waits on ADAPTERS-3 probe (Java ATK app).
- #175 STRICT-TRACE-CLOSE-1 (8903f102): lead review MERGE.
- #174 C3 window-space bounds hash (56e8a941): lead review MERGE.
- #169 (3adb3c94): review fixes done (baseline 1 MB cap, no fsync via #170, docs); release timing main vs branch: no measurable cost; carries #170 commits (merge #170 first). Re-review MERGE (18:10). SKILL.md default line held back: D showed no turn/token gain on 1-2 action tasks, so a first-read line would cost tokens on every call; it stays in commands-observation.md.
- #168: CI red was a real test bug after #167 (unknown-skill message now >400 chars, the stderr line cuts it); lead fix 74c37581 (compare the first 120 chars).
- #162 (b601fd16): all review findings fixed + main merged (web-ref semantic clash resolved); sent control READY 17:45. wait-decidable now on C3.
- 17:25 merged origin/main into #157 #158 #160 #163 #164 #165 #166 #168 #170 (all MERGEABLE, CI re-running). Merged by control: #161 #167 #159 #156 #154 #155 #152 #149 #150 #147. STATUS sent 17:30.
- #170 SNAPSHOT-FSYNC-1 (bcaf4e6b): lead review MERGE. #169 will depend on it (write_private_cache_file).
- Lead review 15:45: #156 MERGE (pre-commit release rule, subfolder refusal, missing-path FAIL, fixtures; adds #137/#139 fragments), #157 MERGE (core default delegates preflight_permission_report to permission_report, so macOS/Windows unchanged; main.rs shrinks), #158 MERGE (isolation default + inherit opt-in; selftest covers both). Waiting on CI.
- #161 focus_phase flake (head 1042a6e2): sent to control as READY (CI pending) 15:58. Lead review MERGE.
- Sent control READY batch 15:58 (CI pending): #161, #159, #160, #157, #158, #156.
- #162 item C + C14 (efa3f57a, +lead docs commit 615ba3ea: json-output early ends, apps/docs 10 s default): second-worker review FIX (3 blocking: unresponsive end not evidence-based and too wide; per-poll uncapped window listing; docs contradictions) -> sent to wait-decidable 16:30. C3 next as its own PR (ref_action_poll.rs + one line in ref_action/mod.rs:196).
- #163 G3 (eec80633, +lead docs commit f653fe91): lead review MERGE. G4 test drafted at ~/.cache/agent-desktop-target/find-notes/; FIND-TEXT-SLOW-1 analysis only (likely: full evidence on every node since #105; wait --text runs a full snapshot per poll at max_depth 10).
- #164 LOCK-LABEL-2 (8b93e0ab): lead review MERGE (expiry_error picks deadline only at contention 0; core + windows). #158 got a lead docs commit e8cd83fe (CONTRIBUTING.md). STRICT-TRACE-CLOSE-1: diagnosis only (unknown disposition + INVALID_ARGS close settles to not_delivered/safe in src/dispatch/disposition.rs; fix in command_scope.rs complete()).
- #165 C8 linux headed-click fallback (c91303f4): lead review MERGE (falls back only for pointer_target_not_visible before any press; keeps refusal when no semantic verb). Not verified on LibreOffice (no VM).
- CI reruns queued for the known focus_phase flake: #158, #160 (both red only on it); #161 macOS E2E AE4 rerun queued.
- #166 Windows CI flakes (fbf0db8c): lead review MERGE (budgets made not-the-thing-under-test; reap removes the manifest only; U7 pays powershell cold start). 3 CI proof runs: lead's proof-runs.sh -> .agents/lead15/proof-test-windows-ci-flakes-2.txt.
- G (Windows A/B, via windows-capability-probe on wip/windows-ab-dispatch, run 43652633673): fork 1.4.0 vs upstream 1.3.1 within ±3% on every leg (WinForms snapshot 589 vs 579 ms median; WPF click 1586 vs 1586). No fork gap on Windows. Finding: `--headed type` fails on the hosted runner on BOTH arms (ACTION_FAILED 'lost focus before physical input delivery') - next Windows item. perf/windows-competitive-latency (87ca1e9c) needs FINDINGS row + fragment + PR after #154.
- #167 item B skill diet (74fc0745): Linux 28,157 -> 8,383 B; t1/t3 input -59%/-49% at 3/3 pass; general 28,000 B test. Review FIX (blocking: 'delivered_unverified' wording says the effect is proven; nits: STALE_REF capture_incomplete pointer, wrong Linux overlay topic, docs, unknown-name error, precedence test) -> fixed at 02ebd653 (Linux 8,927 B, Windows 10,162 B, macOS 23,986 B); re-review running. Sent control a pending note + Mac skill draft (.agents/lead15/outbox/agent-desktop-macos.SKILL.md). Open asks: since-diff decide on collapsing nameless group wrappers in --lines (~40% of GTK4 lines); add stderr bullet after agent-errors lands.
- #168 C15 stderr summary line (72fdd36b): lead review MERGE (stdout unchanged; one function report_response_errors for a future daemon client; AGENT_DESKTOP_STDERR_SUMMARY=0 opt-out; batch per-entry lines). SKILL.md sentence to forward to skills-tokens after it lands. agent-errors items 2 (C4/C12, draft at ~/.cache/agent-desktop-target/agent-errors-item2-refusal.rs.draft) and 3 (C9) not started.
- #169 item D snapshot --since (cbedf68a): opened; post-action read bytes -81/-82% on gnome-text-editor, but turns/tokens not lower on 1-2 action tasks (n=3). Windows CI red: golden JSON CRLF -> lead fix pushed (normalize line endings). Review FIX (blocking: every plain snapshot now writes a baseline with 2 fsyncs, unmeasured vs main; docs miss diff_baseline.json) -> since-diff.
- #162 re-review: 9/9 fixed; one new (pre-dispatch baseline ignores the action deadline, repeats per retry) -> wait-decidable.
- #160 G1 (head 5d467568): lead review MERGE (refuse_unproven_refs reuses refused_before_reading before the lease; hover+drag; tests record zero adapter calls). Waiting on CI. CI red (Linux + Windows): Test (Linux) contract_command_surface_tests missing the new helper module; lead fixing (one line).
- #159 web ref -w (head b2523124): lead review MERGE (one-line surface_target fix + 2 tests, invert-verified by worker; docs/skill text matches). Waiting on CI.
- #142 CHANGELOG-FRAGMENTS-1: MERGED by control. #143 G2: MERGED.
- focus_phase flake (control: urgent): worker `flake-focus`, branch test/focus-phase-virtual-clock.

## First look at the bench (lead, 2026-11-14)

Linux native, Sonnet 5.5, n=5, medians (all runs | pass-only). Upstream ad-up vs fork ad-14:

| task | up pass | fork pass | up wall | fork wall | up tok k | fork tok k | up turns | fork turns | up tool | fork tool |
|---|---|---|---|---|---|---|---|---|---|---|
| hn1 | 3/5 | 4/5 | 51.0 | 65.5 / 55.6 | 308 | 341 / 322 | 17 | 18 / 16.5 | 23.7 | 28.3 |
| hn2 | 3/5 | 4/5 | 32.9 / 31.9 | 42.4 / 41.0 | 218 / 205 | 343 / 328 | 14 / 12 | 17 / 16.5 | 11.5 | 17.6 |
| hn3 | 5/5 | 5/5 | 22.6 | 16.1 | 154 | 96 | 10 | 6 | 8.5 | 7.0 |
| hn4 | 5/5 | 5/5 | 38.7 | 54.4 | 243 | 295 | 14 | 15 | 18.6 | 25.4 |
| hn5 | 5/5 | 5/5 | 43.6 | 40.4 | 444 | 365 | 21 | 19 | 12.7 | 11.7 |
| hn6 | 5/5 | 5/5 | 37.9 | 38.4 | 322 | 276 | 16 | 14 | 14.3 | 14.3 |
| hn7 | 5/5 | 5/5 | 44.0 | 61.2 | 157 | 260 | 10 | 15 | 32.8 | 36.4 |
| hn8 | 5/5 | 5/5 | 47.8 | 52.9 | 127 | 226 | 8 | 14 | 33.6 | 34.2 |

Losses: hn1, hn2, hn4, hn7, hn8 (more turns). Second-call input: upstream ~14.1k, fork ~15.5k, Cua ~5.4k.
The fork's first call is `skills get desktop; skills get platform` = 28 KB of output.


## RESUME HERE (paused 2026-11-14 ~21:30 PST for the subscription limit; resumes after 01:00 PST Sunday)

Control's order after the reset: #182 (XTEST crash), #177, #178 first. Rules in force: <=4 laptop workers; all cargo via
~/bin/build-slot; free -m >= 3000 MB; narrow gates, CI is the full gate; no main merges into open PRs unless
CONFLICTING; push a PR once; no new PRs while >=10 laptop PRs are open (control waived it for #181, #182).
Monitor: re-arm `bash .agents/lead15/watch-prs.sh` (PR list in .agents/lead15/watch-prs.txt; state in watch-seen.txt).

Open laptop PRs (head, state):
- #182 fix/linux-xtest-key-crash 52491b1d: release-blocker fix; second-worker review FIX (Wayland path unheld; signal path unheld; holders unbounded/FFI not flat; + should-fix) saved in .agents/lead15/review-182.md -> send to linux-losses (a51391e0385d616ca) right after the reset, one push.
- #177 fix/linux-launch-still-starting cb5b2a13: all review findings fixed, lead accepted; CI pending -> READY when green.
- #178 fix/actionability-refusal-next-step caa854a6: re-review MERGE; CI pending (first real run of the changed macOS E2E AE7) -> READY when green; nits only if another push is forced.
- #162 fix/click-wait-decidable b601fd16: reviewed MERGE; next in control's merge order.
- #169 feat/snapshot-since-diff 3adb3c94: reviewed MERGE; #170 is merged, so it can go.
- #181 fix/wait-text-depth (head after lead PR-number commit): reviewed by lead; Mac check by control.
- #158 e94784eb, #164 6ff4d7b1, #166 945354e6, #168 74c37581, #173 01aedc5f, #175 8903f102, #176 e6637789: reviewed MERGE, CI.
Merged today: #142 #143 #156 #157 #159 #160 #161 #163 #165 #167 #170 #174.

Parked branches (no PR; open when <10 laptop PRs are open):
- perf/find-text-evidence 7e71822f (FIND-TEXT-SLOW-1): review fixes 1-7 pushed (probe depth 50; --text-only reduced plan; probe stops after the adapter reads unasked fields). Left: full core tests, clippy, windows cross-check, stale-ref script; wrapper-depth test (needs a wrapper-marking test tree helper); Linux re-measure with #181; Mac A/B; body ~/.cache/agent-desktop-target/pr-body-find-perf-find-text-evidence.md.
- perf/linux-keyboard-one-session 34c4c4c9 (ADAPTERS-10): not reviewed; body ~/.cache/agent-desktop-target/pr-body-linux-losses-perf-linux-keyboard-one-session.md.
- docs/skill-stderr-pipe-note 90db71c2 (wip): skill line nit (optional suggestion; no `2>&1 | jq`) + #178 nits could join it.
- OPENED 22:00 (laptop PRs dropped to 8 after merges of #158 #162 #164 #175 #176): #183 fix/windows-type-focuses-field and #184 test/ffi-session-home-race. Neither has a second-worker review yet -> review after the reset.
- fix/windows-type-focuses-field d5cdca55 (HIGH): Windows headed `type`/`press <ref>` now SetFocus an unfocused field first (+ E2E leg moves focus away first). CI A/B run 43670582799: branch 7/7 WinForms + 7/7 WPF; fork main 0/7, upstream 0/7 ("lost focus"). Body ~/.cache/agent-desktop-target/pr-body-windows-fix-windows-type-focuses-field.md. Skill line for skills/agent-desktop-windows/SKILL.md + apps/docs windows.mdx: "Headed, `type` focuses the field itself when it lacks keyboard focus (a `UIElement.SetFocus` step before `SendInput.type_text`); no separate `focus` call is needed."
- test/ffi-session-home-race f1322be6: FFI-SESSION-FLAKE-1 fixed (session test now under HOME_LOCK; HOME race reproduced 4/400k without). Body ~/.cache/agent-desktop-target/pr-body-windows-test-ffi-session-home-race.md.
- perf/windows-competitive-latency fe1c86a5: A/B probe; needs a FINDINGS row + fragment before its PR.
- FOUND (Windows): WPF first headed `type` typed its text but reported TIMEOUT post_action_verification after 5.3 s -> an agent retry would type twice. Not started.
- All workers stopped at 21:50 PST.

Queue after the three: ACTION-7 (wait-decidable, after #162 merges); ref_action_poll timeout() disabled suggestion (wait-decidable); #157's linux.mdx display row (small docs PR or fold into a later docs PR);
batch `launch` PERM_DENIED on a dead display (needs a PermissionReport field across adapters); D ref-action opt-in (low value per
D's measurements); since-diff group-wrapper collapse (check single-child collapse in ref_alloc.rs first); macOS skill draft
PR for a Mac re-bench (.agents/lead15/outbox/agent-desktop-macos.SKILL.md); Linux native bench re-run on the VPS (control runs it).
Worker agent ids: see "Agent ids" above (+ agent-errors ad84534b66855f148, flake-focus a1ade0672618ffdef).

## Item B targets (control, 2026-11-14 ~15:00)
- Bash tool spills output >~30,000 chars to a file (model sees a 2 KB preview); main's `skills get desktop; skills get platform` = 29,285 B. Targets: desktop <= 12,000 B; desktop+platform <= 24,000 B every OS; test fails > 28,000 B any OS (general one; Mac PR #148 may add a macOS-only duplicate -> tell control). Line near top: write flags out in full, no shell-variable flags, never discard stderr. Sent to `skills-tokens`.

## Agent ids (for resuming after a session restart)
hygiene a885640e0014b9341, linux-launch a5a593cac1f10169b, wait-decidable a2bffcbd8cfcae533, skills-tokens a23dda3026421b80a,
since-diff a0a485a1a624871a5, daemon aa256d66643813747, linux-losses a51391e0385d616ca, cli-time a575af5fe8a47c221,
windows aa34d5b134f17f2ae, web-ref-wait a492ed050de67e0f0, hover-raise a9782ecf6825768d0, find a36b7a33aca7c5903,
ledger-speed a072fafdbeb3ab028, flake-focus a1ade0672618ffdef, agent-errors ad84534b66855f148.
Restarts so far: ~13:30, ~14:30, ~15:10 PST.

## Blocked on George

- (cleared 16:15) gh token now has `workflow` scope; workers push workflow changes themselves.

- boxd CLI is not signed in on the laptop: no VM for a LibreOffice/GIMP re-run of the held-out bench or for C10/C11 (George: `boxd auth login` on the laptop).

- Leftover throwaway HOME `/tmp/tmp.GN4o1ntsGP` (632 MB in the RAM tmpfs) from worker web-ref-wait; lead's rm was permission-denied. George (laptop): `rm -rf /tmp/tmp.GN4o1ntsGP`

- Leftover empty dir `/tmp/agent-desktop-test-runtime.e4y1Hw` (rm denied for worker).

- Found: hosted macOS E2E AE4 failed on #161 (test-only diff; release code identical) with second_focused=True, so `open_output.ok` or `focus_output.ok` was not True: likely a flake in tests/e2e/scenarios/acceptance.sh:106-124 (fixed sleeps 0.8/0.3 s, no tool wait). Re-ran once. Row: E2E-AE4-FLAKE-1 (fix: wait with the tool's primitives, print both envelopes on failure).

- FOUND (flake): Windows `Test (Windows)` ffi test `session_adapter_keeps_exact_ref_missing_generation_fail_closed` (crates/ffi/tests/c_abi_actions.rs:141) got a null adapter from ad_adapter_create_with_session("test-session-01") on #178 (diff unrelated). Row FFI-SESSION-FLAKE-1 for the windows worker.

## Changed

## Found
- Open PRs not mine that may conflict: #133 (core snapshot loading, cli dispatch), #132, #134; #18, #34 drafts.

- 2026-11-14 two session restarts (~13:30, ~14:30) killed all workers' background commands; all resumed. Workers now commit+push WIP every 20-30 min (RULES.md).
