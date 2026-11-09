# TASKS: perf/speed-cost

## Session 2 (2026-11-08 from 11:40 PST, release night)

- Rule (control, 2026-11-08): a tool call waiting on George's approval, or a decision of his that blocks work, goes to control at once as `[speed] APPROVAL WAITING: <what, why, recommendation>`. Never put George's email or any personal identifier in a User-Agent, URL, header or payload.
- [x] 1. E2E `AGENT_DESKTOP_E2E_SKIP_PRIVATE=1`: #61 squash-merged (main efdc2442, main CI green).
- [x] 2. #59 squash-merged (main 7921e3f6, main CI green).
- [x] 3. Privacy gate, model-free (`bench/privacy_gate.py`, through probe_slot, counts only, throwaway AGENT_DESKTOP_HOME). Result: **GATE FAIL**. Finder and TextEdit stay blocked from model runs.
  - TextEdit Save As on the seeded doc opens **expanded**: its sidebar outline holds 26 tree items (6 equal a personal folder title) and 23+ cells (5 personal titles, 1 iCloud). Collapsed: 0 rows.
  - Finder Go to Folder on open: 0 rows. After a full bench path is set in the field: 1 non-bench row (`-i`), 4 in the full tree, 2 of them static texts equal to a personal folder title.
  - Not gating but found: every Finder window `-i` snapshot holds the sidebar (13 personal-title texts, 1 iCloud, about 44 other sidebar texts). Every Finder model run has sent those names.
  - Options for George: (a) retire finder and textedit model runs (the release claims don't use them); (b) bench sets the Save panel collapsed and restores the pref after, plus hides the bench Finder window's sidebar; neither stops a model from expanding it. Recommendation: (a) for tonight.
  - **George OK'd, 2026-11-08 ~11:48 PST: finder and textedit (stock and seeded) model runs are RETIRED until he says otherwise.** Only weather, settings and calculator run with a model. Control's follow-up idea: a separate empty macOS user for those two tasks, set up by George.
- [x] 4. F9 vs #32: probe `~/agent-desktop-bench/probe-wintitle-main-95fea935/probe_wintitle.py` on main (d2e406cb binary; window code is the same in 95fea935), Screen Recording denied, Finder: 15/15 pass, covering every F9 path (scoped and global list-windows, snapshot by window id, snapshot --app with no id, snapshot with no --app through the focused-only listing, focus-window --title and --window-id strict resolve). #32 misses no case F9 covered; nothing to port. Branch kept.
- [~] 5. Release PR #62 (chore/release-1.3.0, head fefe4ab8, draft): reviewed. Cargo.lock changes only the 7 workspace versions; publish-npm gated to upstream; prepare-release sees npm 1.2.0 != 1.3.0, so the fork gets tag + GitHub release with npm skipped. To do: the changelog's E2E "Testing" line stays only if the post-freeze E2E passes; add a Known issue for the find locator_incomplete bug if its fix misses the freeze; drop each web known issue whose fix merges; remerge main after the freeze. George re-enables Release right before merging.
- [x] 7. Speed loop round 1: skill line "brace a ref built from a shell variable" (PR #63, docs/skill-zsh-ref-modifier). A/B zsh1-3: sum of medians 83.0 -> 59.3 s, 243K -> 213K; calculator 68K -> 41K tokens, `$S:eN` used 3/3 main vs 0/3 fix. Lower median: loop continues.
- [x] Release A/B at n=5 done 13:07 PST (main c6bf3072 vs upstream, 30/30 pass): sum of medians 60.8 s / 215K vs 51.5 s / 176K (+18%, +22%). No Mac speed claim in the changelog. #63 removed the $S:e trap (0/15 main vs 4/5 upstream calculator runs); per-turn cost equal; the gaps are extra model turns from slips on both builds, and the weather and settings signs flip between A/Bs (noise). Plan was: after #63 is on main and before 22:00 PST, rebuild wt-final at main, run `bench/plan-rel5-ab.txt` (upstream b43ac4f6 vs main, 3 tasks, n=5, rotated, one round per lock turn). A Mac speed line goes in the changelog only if main is better at n=5; settings/weather differences at n=3 were noise, calculator is the causal win.
- [x] 7. Speed loop round 2: #67 squash-merged 13:38 PST (main 0bf430c2): root cause #33, false cycles on released ancestors. Live probe fix 10/10 vs main 9/10; perf compare p50 flat, fixture snapshots complete 10/10 vs main 4-6/10 (script exit 1 is its gate failing on main's side). Changelog line added to #62 (11104ea6). Model A/B fnd1-3 (main c6bf3072 vs 0bf430c2, n=3, 18/18): sum of medians 59.9 -> 54.0 s, 204K -> 192K; settings flat (18.5 -> 18.0 s), because no pre-fix run hit the find failure this time, so the gain is noise. Speed loop paused after round 2: the remaining waste is model exploration (weather: `head -c` truncation, then jq re-reads), with no app-agnostic tool fix in sight; the desktop stays free for the browser writers.
- [~] (was) 7. Speed loop round 2: fork regression in `find`. On System Settings, main's `find --name General` returns TIMEOUT `locator_incomplete` in 0.3-0.7 s in 3 of 5 cold passes (upstream 0 of 5), with no limit hit, no transient cause (`bench/probe_find_settings.sh`, `bench/probe_find_err.sh`, counts and redacted text only). Opus writer on fix/find-incomplete-evidence (T3 delegate), started 12:58 PST.
- [x] 7. Speed loop round 3 (George's go, 14:20 PST): #70 skill "Fewer round trips" (Font Book examples, verbatim in zsh 2/2), squash-merged 14:57 (main d4734035, main CI green). A/B n=8 per task per side, Opus 5.5 cfg a, rotated: main a0d644b4 vs #70 build: sum of medians 51.8 -> 42.8 s, means 51.8 -> 43.5 s, median tokens 175K -> 147K, turns 12.0 -> 10.5, pass 22 -> 24 of 24 (weather 19.0 -> 11.6 s; settings 18.1 -> 17.5 s; calculator 14.7 -> 13.7 s, tokens up 40K -> 57K). Upstream b43ac4f6 vs #70 build: 51.6 -> 42.7 s medians, 51.4 -> 44.1 s means, 183K -> 147K, 23/24 each; calculator slower than upstream (12.7 -> 14.9 s). Measured line added to #62 (c52588ae+). Plans: bench/plan-skill-ab.txt, bench/plan-skill-up.txt.
- [x] 7. Speed loop round 4 (calculator): #71 (skill: look refs up by name with jq) A/B vs main 1534ccf9, n=8 per task per side, rotated, 16:05 PST. Calculator 13.8/14.8 -> 12.2/14.1 s (median/mean), 49K -> 42K, 6/8 -> 7/8; weather 11.8/13.2 -> 10.5/14.0 s (one 8-turn 34 s run); settings flat. Sum 42.1/44.3 -> 39.3/44.5 s, 140K -> 120K, 22 -> 23/24. The mean rose 0.2 s, which fails the merge rule, so #71 is CLOSED. Only 1/8 calculator runs used the jq lookup; the 9-for-7 slip fell 7/8 -> 4/8. Plan: bench/plan-skill2-ab.txt.
- [x] Item 1 (post-release, NOT 1.3.0): #74 draft `--lines` (Opus writer, 7a561c99). A/B vs main c2e24adb, n=8 per task per side, rotated, 16:57 PST; all 24 #74 runs used --lines. Median/mean: weather 14.2/14.8 -> 12.3/11.9 s, 30K/41K -> 28K/28K; settings 21.8/21.8 -> 16.4/18.1 s, 61K -> 51K; calculator 21.9/20.8 -> 18.5/17.2 s, 57K -> 53K, 8/8 -> 7/8 pass. Sum 57.9/57.4 -> 47.2/47.1 s (-18%), 148K/157K -> 132K/131K, turns 10.0/11.00 -> 10.0/9.88, pass 24 -> 23. Main ran slower this round than at 1534ccf9 (42.1 s sum), so absolute times drift with machine load; the pairing is what counts. The calculator 9-for-7 slip persists under --lines (7/8 first loops vs main 6/8): the model builds key refs from the keypad pattern, not by reading, so neither output shape nor skill text fixes it.
- [x] Release A/B with #70+#74 (18:16 PST): upstream b43ac4f6 vs main b1ee1737, n=8 per task per side, rotated; all 24 main runs used --lines. Median/mean: weather 16.5/22.4 -> 11.5/12.1 s, 60K -> 27K; settings 16.1/18.1 -> 18.2/16.7 s, 75K -> 50K; calculator 13.2/15.0 -> 15.3/15.2 s, 40K -> 53K, 8/8 -> 6/8. Sum 45.9/55.5 -> 45.0/43.9 s, 174K/201K -> 130K/129K, turns 12.0/13.6 -> 10.0/9.75, pass 24 -> 22. 9-for-7 first-loop slip: upstream 2/8, main 7/8, which is far more often on the fork and worth a post-release look (the skill's chained launch output or the ref-heavy guidance may prime it). #62 Performance line replaced (b6293930+). Plan: bench/plan-rel-lines.txt.
- Found: TextEdit's cmd+shift+s is Duplicate, not Save As (cmd+shift+alt+s). Closing the duplicate shows Delete Copy/Cancel/Save; macctl's discard list lacked "Delete Copy", so the hygiene clean failed 3 times. Fixed in bench/macctl.swift.
- Found: the old "fork 25% faster, 31% fewer tokens on hab1-4" claim does not reproduce. `python3 bench/headline.py` over hab1-4 gives the suite upstream b43ac4f6 653 s / 3.06M against fork 30e1503b 690 s / 3.90M. Weather+settings+calculator alone: 57.4 -> 41.3 s (-28%), 258K -> 163K tokens (-37%). A fresh A/B on main 95fea935 vs upstream for those three (`bench/plan-rel-ab.txt`, rounds rel1-3) runs from 11:49 PST for the changelog.
- Web numbers for the changelog (browser CHECKPOINT 11:46 PST, `~/agent-desktop-bench/bench-web-rerun-a8b03477/summary.txt`): ad-web on main 95fea935, 15/15, n=3, sum of medians w1-w4 32.8 s / 138.4K input tokens, vs AX-only 166 s / 952K (older run) and agent-browser 47 s / 171K (older run). x2 tokens up ~34% vs #47, not investigated.
- Release changelog (control, 2026-11-08): web figures say "local test pages" (real-site round 0 has the web channel much slower than agent-browser). Known issues: (1) a snapshot of a very large page can fail with INTERNAL "RefMap exceeds 1MB size"; (2) clickable rows exposed as plain text get no ref. The browser session's Opus writers fix both, due 23:00 PST. Drop each line only if its fix merges before the freeze. Two more writers share the build queue tonight.
- Found (2026-11-08 14:32 PST, confirmed 2/2): on main a0d644b4, `click <unselected Font Book sidebar cell or treeitem> -w ":English"` returns ACTION_FAILED "Selection was delivered but its effect could not be verified" with delivered_unverified, though the row is selected (window title changes). A plain `click` on the same unselected row returns delivered_verified. Post-release: the selection postcondition seems to read differently under `-w`.
- Found (2026-11-08 17:22 PST): on #74 head d69c68c0, the Windows CI test `input::clipboard::tests::hung_delay_owner_returns_app_unresponsive` failed with "OpenClipboard failed for delayed owner: 5" (access denied on the shared runner). #74 touches no Windows code. Failed job rerun once; if it fails again it gets its own small test PR under the flaky-test rule.
- Found: macOS has no `setsid`; long runs go through a Python `os.setsid()` wrapper with nohup.

## Post-release plan: turn count (written 2026-11-08 ~14:20 PST, no runs tonight)

Source: the 60 runs on main builds today (`wt-final`, `wt-main67`), 20 per task, 60/60 pass, from `bench/runs/results.jsonl`.
Labels from `python3 bench/turn_causes.py`, then checked by hand against the commands. Ideal is 3 turns per task (one
look, one act-and-read, the answer); every turn past that is counted. Some `wt-final` calculator runs predate #63.

| Task | Median turns (min-max) | Extra turns | Causes, with counts |
|---|---|---|---|
| weather | 5 (3-7) | 36 | 18 re-read after `head -c` cut the snapshot; 5 first look too shallow (`--skeleton`, or `find --role cell` with no match), then a full snapshot; 5 extra `--root` drill or re-drill; 3 `launch` alone in its own turn; 2 `--help`; 1 find to locate the list; 1 re-filter of the same tree; 1 turn with no tool |
| settings | 6 (5-9) | 60 | 14 find retries (`find --name General` first, then About, or a role/timeout tweak); 13 `--help` (12 `wait --help`), plus 6 click turns split from the read for the same reason; 11 full `-i` snapshot (15 KB) to locate About after finding General; 5 re-read after `head -c`; 4 re-filter; 3 post-click re-read; 3 `launch` alone; 1 no tool |
| calculator | 4 (3-6) | 22 | 12 wrong key (the model typed `9` for `7` or swapped `7 8`; the snapshot names every key correctly, e.g. `e7` "7" / `Seven`), each costing one rerun; 9 zsh unbraced `$S:e…` (pre-#63 runs only; #63 fixes it); 1 re-read |

Candidate fixes, ranked by turns they could remove (per 20 runs). Each is measured post-release as its own A/B against
main on weather, settings and calculator, **n≥8 per task per side**, order rotated, under the lock; primary metric
median turns, then wall and input tokens. Keep a fix only if median turns drop on the task it targets and no task
regresses.
1. **Snapshot output shape: a line-per-ref text form** (e.g. `@sX:e7 button "7"`, children indented), opt-in flag,
   JSON stays the default. Targets the 23 `head -c` re-reads, 5 re-filters, the 11 locate-by-full-snapshot turns,
   and plausibly the 12 key slips (a long JSON line is where the model mis-maps refs). Cost: a contract addition
   (`docs/json-output.md`, skill, golden fixture). Biggest potential; app-agnostic.
2. **Skill text: wait and read after an action.** One worked example `click <ref> && wait --app X --text <t>` with
   the real flag names (models guessed `--timeout`, `--text` placement). Targets 13 `--help` turns and the 6 split
   click turns. Cheap; no code.
3. **Skill text: find the target, not its parent.** "Use `find --name <the thing you will click>`; a parent's
   pane is usually already open." Targets 14 find retries and 11 locate snapshots on settings. Cheap.
4. **Skill text: never truncate a snapshot with `head -c`; narrow it with `find`, `--root` or `-i`.** Targets the
   23 truncation re-reads if fix 1 does not land. Cheap.
5. **Skill text: chain `launch X && snapshot --app X -i --compact` in one call.** Targets 6 launch-alone turns.
6. Calculator key slips with no output change: none app-agnostic in skill text beyond "read the key names back
   from the result before answering"; measure only together with fix 1.
Order of measurement: 2+3+4+5 as one skill PR first (cheap, one A/B), then fix 1 alone against that.

## Handoff (2026-11-08 ~19:45Z, speed session → fresh session)

Machine: the Mac. Bench worktree: `/Users/Shost2/.t3/worktrees/agent-desktop/perf-speed-cost`, branch `perf/speed-cost`. Control thread: `mcp:1f6d7f49-e90f-4bb7-8055-7282273fec73` (env `14d05056-c993-4b71-9ef4-75b0e02fc276`); send CHECKPOINTs there.

### In flight
| item | branch | SHA | state |
|---|---|---|---|
| #59 trace timing flake (test only) | test/trace-budget-windows-flake | 44faf629 | main 95fea935 merged in at 19:40Z (main's lib.rs changed, an always-overlap file). CI running. Earlier head e972d52d passed Test (Windows) 4 times. Merge when green and CLEAN. |
| E2E `AGENT_DESKTOP_E2E_SKIP_PRIVATE=1` (control's plan item 1) | test/e2e-skip-private | not pushed | The Sonnet writer was cancelled within minutes of starting. Nothing pushed. Restart from main. Brief: tests/e2e/** only. |
| F9 window title from AX | fix/window-title-from-ax | 2e5dd2f2 | Decided: keep main's #32 and drop F9. Still to do: probe whether #32 misses a case F9 covered, and port only that. Don't delete the branch. No PR. |
| Bench | perf/speed-cost | see the git log | Final-run results are in bench/RESULTS.md. |

Control's plan (not started): (1) E2E skip-private writer; (2) privacy gate; (3) #59 and F9 probe; (4) release prep by 22:00 PST on chore/release-<next> (version now 1.2.0), with no npm and changelog claims only from measurements; (5) after the 00:00 PST freeze: E2E with SKIP_PRIVATE and EXCLUSIVE, detached, then perf-baseline-compare; (6) speed loop on weather, settings and calculator only, with the stop rule. Release PR green by 07:00 PST Monday.

### Open PRs (gshost1/agent-desktop)
- Merge-ready, older: #5, #7 and #9 (ready), #36, #38, #41, #48 and #58 (draft, CLEAN). Each needs the merge rule: main merge-in, CI green, no overlap.
- DIRTY (need a remerge or a decision): #14, #18, #30, #34.

### PRIVACY gate (blocks model runs of finder and textedit)
- Snapshots of TextEdit's Save panel and Finder's Go to Folder sheet held non-bench row names in 47 runs. Those artifacts are deleted, and text fields are stripped from their results rows.
- Until a model-free gate (under the lock, counts only, never names) shows 0 non-bench rows in both sheets, only weather, settings and calculator run with a model. George still owes an "ok".
- Count method: load each refmap's `inner`, filter on `source_surface == "sheet"` and role cell/treeitem with a name, then match names against bench patterns only. Never print names.

### Bench files a newcomer must know
- `bench/lockq.sh take|release "<owner>" 5`: the desktop lock `~/.agent-desktop/bench.lock`. Take it for anything that drives the GUI.
- `bench/serial_cargo.sh <cmd>`: every cargo command on this Mac, one at a time, with a per-tree target dir under `~/.agent-desktop-bench-target/trees`. It waits while the desktop lock is held.
  - `cargo build --tests` and `cargo test` are separate calls.
  - The pre-commit hook goes as `serial_cargo.sh bash .githooks/pre-commit`, then commit with `SKIP_PRECOMMIT=1`.
- `bench/cargo_env.sh` (`bench_build <tree>`): copies the release binary to `<tree>/target/release/agent-desktop`, with `agent-desktop.commit` beside it. Source it from bash, never zsh, because it relies on BASH_SOURCE.
- `bench/check_binary_tree.sh <tree>`: fails when a binary embeds another tree's crates.
- `bench/orchestrate.sh <plan> <plan>`: runs rounds of `task:config:model:tree`. It takes the lock per round and requires check_binary_tree. Run it detached (nohup) so a T3 restart can't kill it.
- `bench/probe_slot.sh <owner> <timeout_s> <out> <cmd...>`: the lock, the probe, then `bench/post_probe_check.py` (hygiene; `--clean` closes leftovers).
- `bench/BRIEF_RULES.md`: copy it verbatim into every writer or probe brief. `bench/README.md`: privacy, Go to Folder and hygiene rules.
- `bench/out/macctl` (from `bench/macctl.swift`): find windows by AX title, raise, key, close-discard. `bench/out/sheetctl` (from `bench/sheetctl.swift`): present, where-is, expanded, toggle, cancel on a sheet.
- `bench/check_save_folder.py <binary>`: the TextEdit Save panel folder check, under the lock with TextEdit quit.
- Merge helpers: `mergeq.sh` and `remerge1.sh` lived in the old session's scratchpad, not the repo. Re-create them or check by hand: `git merge-tree --write-tree origin/main <head>`, name-only overlap since the merge-base, the always list.

### Known traps
- Never AppleScript System Events. No TCC changes. Never read secret env files.
- Folders: bench folders only (`~/agent-desktop-bench/bench-*`, `probe-*`, `t3-*`). Close any personal-folder window without reading it, and log a count, never a title.
- Sheets:
  - A window with a sheet ignores its close button. Use sheetctl cancel first.
  - TextEdit can stack two sheets, and close-discard opens a Duplicate/Discard/Cancel sheet (macctl now presses Discard).
  - Escape doesn't dismiss an expanded Save sheet.
- Main CI: each push to main cancels the previous main run. `gh run rerun --failed` works only after the whole run completes. PR-watch "conflicts" notices are often stale; verify with merge-tree.
- Flakes seen once each: the Windows lib-test hang, the trace budget tests (fixed by #59), Linux `snapshot_completion_log_omits_private_app_and_window_names` (tracing callsite race; a second failure means a test PR).
- A T3 restart kills background shell jobs. Anything long runs detached.
- Struct fields ≤ 7 and function params ≤ 5: no script checks them. Count before pushing.
- zsh: `$S:e3` expands `:e`; `set -- $spec` doesn't split; arrays are 1-indexed. Use bash -c or Python.


Goal: make agent-desktop faster and cheaper in tokens for an agent driving the Mac; explain the gap to Aside Computer Use.

## Phase 1: measure
- [x] Build release CLI, confirm `status` (accessibility granted, screen recording denied on purpose)
- [x] CLI latency per command per target app, min/median/max over 9 runs, warm-up discarded (`bench/cli_latency.py`)
- [x] Bytes-to-tokens calibration with real `claude -p` usage (`bench/token_calibrate.py`)
- [x] Code survey of fixed sleeps, post-action reads, snapshot fields, refmap I/O
- [x] Model-loop driver: `claude -p`, Bash only, wrapper logs CLI time (`bench/driver.py`, `bench/tasks.py`)
- [ ] Matrix: 5 tasks x configs a-e x 3 runs on Opus 5.5, plus Sonnet 5.5 config a; now run by `bench/orchestrate.sh` one round per lock turn (r2b running)
- [ ] Step-4 breakdown: time (CLI / model / other) and tokens (snapshot / history / skill+system / other)
- [ ] Ranked fix list with estimated wins and prior art (upstream issues/PRs, fork PRs/branches, upstream/main log)
- [ ] Phase-1 report `bench/RESULTS.md` + CHECKPOINT (no wait for a go; fix loop runs)

## Fix loop
- [ ] perf/outline (rebased 3fe63b8a + review fix 5912b5d8, pushed): review done; matrix A/B in ob1–3, perf compare, then PR
- [ ] perf/cold-settle-chromium (64733c9c, pushed): review done. Fixed: Chrome/ego/renamed Electron missed by name list (now V8 snapshot marker; 15/15 Chromium apps on this Mac, 0 native), lazy detect, param shadow, lesson doc, tests (invert-verified). Accepted: non-Chromium async renderers rely on the core retry (unchanged for manual mode), pid reuse between require_core and detect (wrong delay for one activation only). Next: cold A/B in ob1, PR
- [ ] perf/settled-surface-budget (a3462f71): review done and handled; live A/B queued at start of r3 (`bench/ab_surface.sh`); then PR
- [ ] fix/open-released-target (7f90bcd8, pushed): review found 'target released' is not proof (list reloads, closes, crashes). Redesigned: the settle poll holds the target's window and counts a new non-blank title as the effect, on every perform path. Owned-menu readback split out for the 400-line cap. Accepted: wiring of the title read into changed_now has no unit test (needs live AX); covered by bench/check_open_click.sh in ob1. Select-first reorder rejected (#206 sidebar)
- [ ] fix/automation-probe-deadline (c3ba6001, pushed): review done; PR
- [ ] fix/popup-window-viewport (4ef31c29, pushed): carry of upstream #453 (open, joeVenner): pop-up buttons omit AXWindow, so offscreen stays unknown and click/select/expand wait 5 s then TIMEOUT. TextEdit save sheet: ~170 s of 5 s timeouts over 10 runs. Tests moved beside element_bounds for the 400-line cap. #399's vanished-node skip not carried: fork's app_menu treats invalidation as fatal and observe() reacquires. LIVE A/B 2026-11-07 (TextEdit Save sheet, Where pop-up click): base TIMEOUT 5095 ms, head delivered_verified 183 ms. Next: review, PR crediting #453
- [ ] Combined-stack binary (all fix branches merged locally) for the final 5-task run vs Aside

- [ ] Merge gate: before merging anything touching root `Cargo.toml`, check `gh workflow list --repo gshost1/agent-desktop --all`; Release must read `disabled_manually`, else hold and tell the control thread. Checked 2026-11-07: disabled_manually; none of the 5 fix branches touch Cargo.toml/Cargo.lock/npm/CHANGELOG.

- [ ] Merge #4 #5 #6 when CI + fixture perf compare + E2E green (r3b: e2e_branches.sh, perf_compare_branches.sh). Branches contain fork main ec632e97 via merge (no force-push); host tests green on each 2026-11-07
- [ ] Finder rename false success (class A): set-value + headed Return = delivered_verified, name unchanged. Probe v2 in r3b
- [x] fork PR #3 docs save-panel name field: merged ec632e97

- [ ] Upstream policy (George, 2026-11-07): nothing to upstream: no PRs, issues, comments or links that create timeline events. Fixes stay upstream-ready (own branch from main, no browser-work dependency, upstream refs as inline code). Upstream status column in bench/RESULTS.md
- [ ] When squash-merging #5, pass --body without the `upstream `#416`` line from c5f0ec9a's message (inline code `#416` only)
- [ ] Rebuild the open-click window-title signal on top of fix/open-closed-sheet (carry #541) as its third ActivationState field; old fix/open-released-target is superseded

## Beat Aside (goal from 2026-11-07, after the headline A/B)
Target, harness medians over 3 runs: 5/5 verified, total wall < 106 s, total tokens < ~1M.
Budget read from runs so far: Weather + Settings + Calculator ~45 s / ~150K, so Finder + TextEdit must fit in ~60 s / ~850K.
- [ ] Scoreboard in bench/RESULTS.md, filled from the headline A/B; update after each merge, CHECKPOINT when a number moves
Ranked (bench/oss-survey.md has the prior art). Offline trace measure, opus runs so far, per run:
Finder 109 CLI calls, 43 snapshots (11 right after an action), 931 KB of snapshots, 46 s inside snapshot;
TextEdit 62 calls, 26 snapshots (9 after an action), 567 KB, 53 s inside snapshot (Save-sheet reads ~3 s each).
1. [ ] S1 key chord -> AXPress of the menu item with that AXMenuItemCmdChar/modifiers (Cua menu_equivalent.rs, MIT); refuse when disabled. probe1 `menu` lines measure chord vs AXPress for Finder cmd+shift+n and TextEdit cmd+s
2. [ ] F2 Finder rename: write the focused inline editor, then commit (AXConfirm / pid Return / unfocus / press return), probe1 `finder` lines; AXURL on the row recorded for the rename-on-disk idea
3. [ ] F3/S2 Save panel: owning pid (panel service vs app), cmd+shift+g to app pid / panel pid / menu, probe1 `owner` and `goto` lines
4. [x] F1 find --role menuitem falls back to the menu bar (perf/find-menu-items, 4c473b58); unit tests invert-verified; live check + perf A/B = plan round fix1, then PR
5. [ ] F4 Save-sheet and Finder snapshot latency (~3 s each, 46-53 s per run): profile; node cap with truncation note (Cua 250 nodes)
6. [ ] S3/F5 diff read after an action (Cua since:"latest"): 25-35% of Finder/TextEdit snapshots follow an action, 166-275 KB per run
7. [ ] S4 observation to a file + short summary (Playwright CLI, Chrome DevTools MCP, Apache-2.0)
8. [ ] S6/F6 batch-first default in skill; batch stops when a surface appears or focus moves, returns one diff
9. [ ] S5/F7 narrow by default: query with ancestors, collapsed menus, sheet preferred when open
10. [ ] S7 event-driven settle (AX notifications quiet, capped) instead of fixed waits
11. [ ] S8 serialization trims (static text repeating parent name; no ids/bounds in diffs)
0. [~] F8/S9 skill quickstart (delegated: perf/skill-quickstart). Measured here: appended SKILL.md 7% of all input (13-15% on short tasks); `skills get` output 4-8% and `--help` output 8-12% on Finder/TextEdit; doc text ~23% overall. First-call prefix 11.1K tokens, ~9.2K of it harness. A/B: trim vs fork main, config a, 3 runs per task, queued when the branch lands
13. [~] F9 list-windows title from AX when CG gives no name (delegated: fix/window-title-from-ax)
14. [ ] F10 first-snapshot latency
- outline A/B ob1-3 (n=3, config a): Weather 62K->26K tokens, 18.7->11.4 s; Settings 21->16 s, tokens flat; Calculator flat; Finder/TextEdit inconclusive, every miss on both builds is a route failure (untitled folder left, or a stray save). Outline PR waits for the route fixes, then re-measure.
- rename v4 (Get Info via menu AXPress): click delivered_unverified, no Info window appeared, 2/2. Matches the survey: Finder does not commit these routes headless.
- [~] skill quickstart (perf/skill-quickstart 6326b78c after review fixes; 6979->3283 B core, 7771->3045 B macOS): A/B queued as skab1-3. Follow-up: Windows 8349 B and Linux 17219 B platform skills uncapped
Last resort only: SLEventPostToPid (private API), only if CGEvent.postToPid fails a measured case.

## Found
- Bench prefix: each `claude -p` run loads this Mac's user-level skills, agents and slash commands into its system prompt (stream init event lists them). That inflates every call's prefix for both builds alike, and Torben's setup likely had none. Not changed mid-A/B; candidate for a clean-profile run so the comparison against Aside is fair.
- E2E fixture gate fails on every checkout because Screen Recording is off: CG windows have no name, list-windows falls back to the app name. F9 (delegated, fix/window-title-from-ax).
- `~/.agent-desktop` created with mode 755 makes every snapshot fail with INTERNAL "private file parent is accessible by group or other users"; no suggestion in the error.
- Snapshot JSON tokenizes at ~2.0 bytes/token (refs, punctuation), not ~4. A Finder `-i` snapshot is ~16.7K tokens.
- `--compact` is byte-identical to `-i` on Weather, TextEdit, System Settings, Finder: macOS groups carry SetFocus/ScrollTo/RightClick, which blocks collapse.
- zsh footgun: `S=@sid; agent-desktop click $S:e3` expands `:e` as zsh's extension modifier and sends `click 3`.
- `click --wait-for` returns a full unfiltered snapshot (depth 10, no `-i`): snapshot flags can't be passed through.

- Every Finder click/select/expand/press cost ~5 s: post-action `list_surfaces` got the whole action budget; the macOS context-menu walk visits the whole app tree, times out, result discarded. `list-surfaces --app Finder` always TIMEOUT. Fixed on perf/settled-surface-budget.
- ADAPTERS-2 on Finder: list-view row click selects only (no open). Icon/name-textfield click: AXPress skip, AXOpen opens the folder, returns -25205, ACTION_FAILED uncertain.
- list-surfaces: Notes APP_UNRESPONSIVE after ~4 s; System Settings APP_UNRESPONSIVE in 0.25 s.
- Finder 60-folder list view: `snapshot -i` 3.83 s, 518 refs (over the 3 s default budget).

- TextEdit save sheet: pop-up/combobox click, select, expand wait the full 5 s actionability budget, then TIMEOUT `visible: live offscreen state unavailable` (AXWindow -25212 on pop-ups; combobox never publishes AXExpanded, so states incomplete). Prior art upstream #453. Also `find` in the sheet TIMEOUTs at 5 s and sheet snapshots take 3 s.

- Automation for T3 Code to System Events is denied on this Mac (George chose Don't Allow, 2026-11-07). `agent-desktop permissions` reports automation `denied` here: expected, not a regression. bench/guard/osascript blocks any AppleScript to System Events for model runs and the E2E harness, whose fixture-window resize step uses it; the E2E acceptance AE5 denied-Automation path now runs instead of skipping.
- macOS E2E harness needs Python 3.10+ as `python3` (`str | None` in tests/e2e/bounded_process.py) and doesn't check: under /usr/bin/python3 3.9.6 every guarded command dies on import and the gate reports "reviewed binary version command failed". bench/e2e_branches.sh now puts python3.11 first. The same early exit also hits `cleanup_files[@]: unbound variable` (tests/e2e/lib.sh:288, `set -u` on stock bash 3.2). Candidate small fix/ branch: a prerequisite check plus a safe empty-array loop.
- Rename probe v2: set-value plus headed Return gives ok/delivered_verified (`application_commit: not_verified`, `verification_scope: element_value`) and Finder keeps "untitled folder"; headed type --interval-ms 20 gives TIMEOUT (field never focused); per-key headless press plus Return commits but drops letters ("renamd-pres-key"). v3 (round ob2) tests set-value and type with a headless Return.

## Blocked on George
- Nothing.

## Unattended night 2026-11-07 → 2026-11-08 (control room priorities)
- [ ] 1. AX window titles when Screen Recording is denied: every title is the app name; `wait --window` can never match (writer running)
- [ ] 2. Revisions of #24 (rename field unreachable while the inline editor is open), #25 (background menus read as disabled; front lookup incomplete), #26 (probe could not find Where), sheet-find writer; then n=3 per fix
- [x] Finder n=3 rounds fx1–fx3 started (main, #24, #25)
- [ ] TextEdit: Aside-identical task blocked (container pref not writable: "Could not write domain"); tonight `textedit-seeded` (bench-seeded `src/seed-<id>.rtf`, Save As into `dest/`), arms main, #26, #27, plan bench/plan-seeded.txt. Rows never mix with `textedit`
- [ ] 3. Two winners → combined fork build, full 5-task run with textedit-seeded, n=3
- [ ] 4. Finder right-click returns APP_UNRESPONSIVE (AXShowMenu -25204, delivery_uncertain) although the menu opens within 0.2 s: own fix
- [ ] 5. For the laptop's ledger, not fixed here: #18 `wait --menu-closed` with no menu returns after 2.08 s (elapsed_ms 2078/2079, two runs), not ~1.5 s; the closed read that starts after the 1.5 s grace adds ~0.6 s
- [ ] #30 flaky core test fix (gone-wait budget): merge when CI green + CLEAN + overlap check
- Blocked on George: option 1 (grant app-data access so the save-folder pref swap works) for Aside-identical TextEdit rows
- [ ] #26 probe v2 (2026-11-07 23:58): panel opened in bench src ("Where" = src, panel pid 11829); `press cmd+shift+g --app TextEdit` → CGEventPostToPid to TextEdit, delivered_unverified, NO data.details.keyboard_target, no hosted-focus debug lines; Go to Folder field never appeared. The fix's route did not engage (or the probe binary lacks it). Evidence: bench/out/live-probes/pr26-v2-out/. Send to the #26 writer when a writer slot frees.

## 2026-11-08 night, after 01:00 (speed session)
- [x] Shared cargo target dir gave wrong builds: per-worktree target dirs (c6b95637), provenance check before every round (f76160fb). Rows with another tree's core: 9 marked suspect-build (fx main/#25, ts1). hab/fv and fx #24 valid.
- [x] Finder ignored `open <bench dir>` behind a leftover OK alert: bench clears OK-only Finder alerts (5d34c709).
- [x] #24 v2 df5d379e live PASS 4/4; PR #24 ready, CI 13/13. n=3 Finder rounds on this head pending.
- [x] #36 right-click 94416237 live PASS 3/3 (main APP_UNRESPONSIVE 3/3), still 2.8 s per call. Speed follow-up on hold (writer cap).
- [x] #32 window titles 25cbc103: live probe run4 PASS 13/13 checks (snapshot --window-id title too); CI 13/13; body updated. Left draft: the writer's brief says don't mark ready (George or control: `gh pr ready 32 --repo gshost1/agent-desktop`). Open: perf A/B (scripts/perf-baseline-compare.sh) vs merge-base, needs the desktop.
- [ ] #41 (ae2add5c): state-dir permission failures are PERM_DENIED, not INTERNAL. Draft, CI pending.
- [ ] #25 v2: background AXPress on a disabled-reading item reports success, does nothing (no folder, window not closed). TextEdit menu walk deadline at 1.2 s. Sent back.
- [ ] #26: v2 (keys to TextEdit pid) failed; v3 (keys to the panel service pid) DID open Go to Folder: the probe misread `data.match` as `data.matches.0` (verified in out-005819/03-goto-field.json). Corrected to control. 7cd59f4c makes hosted-panel press verify an effect; v4 probe tests the AX route (select Where:, click dest, set name, click Save) on 7cd59f4c+#27, queued.
- [ ] #33 sheet-find: no difference on a 1-file bench folder (0.38-0.48 s both). Needs a 3000-file bench folder probe.
- [ ] Re-runs fxr1-3, tsr1-3 (plan-rerun.txt) running.
- [ ] Option 1 (non-seeded TextEdit, Aside-comparable): pref write still blocked at 01:25 after George's grant (`defaults write` to the TextEdit container: Could not write domain). Not retried; waits for a T3 relaunch in the morning (Blocked on George).
- Note: every writer's local gate run before ~00:45 used the shared target dir and is suspect; CI is the gate.

### 02:10
- [x] #26 v3 key route on 7cd59f4c+#27: PASS (saved-020508.rtf in dest, body ok). bench/out/live-probes/pr26-v3route.txt
- [ ] #26 v4 AX route: Save As + Where Select delivered_verified; probe aborted finding `dest` in a collapsed panel (no file list, 7 refs). Probe now expands via disclosure first; v4b queued (b4pftoaxo). Same abort again: write down, stop.
- [x] #36 387dd2bd: head 3/3 verified, 407 ms median; verify-only 3/3, 2614 ms; main 0/3, 2579 ms.
- [x] Sent #33 large-folder A/B and #25 v3 results to their writers.
- Found: each Finder run makes ~30 model calls at 3-4 s; cli_s 22-50 s of 120-150 s. Under 60 s needs ≤ ~15 calls, so look at the skill/recipe (Go to Folder alone took ~30 s and 8 calls in the passing run).
- [x] Model-call analysis: Finder median ~31 calls, ~70% navigation + rename workarounds. Top cuts: (1) skill recipe, folder window already open → new named folder, on #24, ~8-12 calls; (2) Go to Folder `press return` reports AXConfirm but the sheet stays open: verify the effect / press Go, ~5-8; (3) skill teaches batch + one-line synopses, ~3-5. Not run, so estimates.
- [ ] Writer for cuts 1+3 (skills, on #24) and one for cut 2 (tool), when a slot frees. Check with n=3 Finder vs main.
- Control: prompt stays. Cuts 1+3 → 5th writer (Sonnet), skills-only, stacked on #24, branch docs/skill-call-recipes, started 02:2x. Cut 2 takes the next free slot, ahead of #36's follow-up. Check: n=3 Finder vs main AND vs #24 alone.
- [ ] #26 v4 AX route: SAME ABORT TWICE (020500, 021137), stopped per the rule. Both runs: Save As delivered, Where pop-up Select delivered_verified (moved to the probe folder). v4b expanded the panel (disclosure AXPress); then `find --surface sheet --name dest` walked 194 nodes, complete (no limit hits, deadline_exhausted 0), 0 matches, x10 over 5 s. So the file row is visible but not matched by --name: its label is likely a cell value/static text, not the row name. Open question for the #26 writer: is that a find gap (rows not named from their cell) or a probe lookup error (use --text)? Evidence: ~/.agent-desktop-save-panel-probe/out-v4-021137, bench/out/live-probes/pr26-v4b.txt. v3 key route PASS stands.
- Control: ts26 rounds stay; stopping v4 is right. Row naming is a general find gap (rows labelled by a child cell/static text, any app). Asked the #26 writer (1118bb44) to answer from the code + v4b evidence, no new probe. If it is a gap: its own small PR later, not #26.
- [ ] Row-naming find gap (own PR, writer later): `label_from_children` (crates/macos/src/tree/child_labels.rs:33-104) ignores AXTextField children, so file-list rows whose label is a text field value get no name; `--name` only matches identity.name (core locator/mod.rs:77). Workaround today: `find --role treeitem --text <name>`. Fix: name row/treeitem/cell from a sole non-secure text-field value, marked derived; test in a second app's file list. Probe first whether click on a named folder row enters it (chain ends at select, likely not).
  BRIEF CONSTRAINT (control): the derived name is for locating (--name) and display only; it must NOT enter the capture proof as a stable-name field (control values are never identity: a rename or cell edit would give false STALE_REF or a false match). The only alternative is a probe showing the value is read-only for that role. QUEUE ORDER: cut 2 (Go to Folder confirm) first, then row naming.
- #26: 13/13 green, draft; writer updating the PR body (v3 PASS, v4 blocked by row naming). George marks ready.
- Found 02:17: fxr3 #24 (409c4fc6) Finder PASS in 34.5 s, 12 calls, cli 7.3 s (run 1108021019-80bc). The agent read #24's "In-place edits" skill section and used `--surface focused` → set-value → return. First sub-60 s Finder row.
- [x] #26 writer done: PR body updated (head 7cd59f4c, 20 hosted_focus tests, v3 PASS, v4 blocked by row naming), 13/13 green, draft.
- [ ] Cut 2 writer started in #26's freed slot (Opus high, branch fix/sheet-confirm-effect, base main or #24 by evidence). Returns a probe for me to run n=3 vs clean main.
- Control 02:2x: control holds the merge right on the fork; George doesn't need to.
  - #26: ready + merge once ts26 n=3 shows no regression vs main.
  - #24: ready + merge once the f24 v2 rounds pass, ahead of the stacked skill PR.
  - Merge rule: main merged in, gates + CI green, CLEAN, no overlap. CHECKPOINT after each merge; watch main CI.
- [ ] E2E on #32 (25cbc103), control 02:3x: browser saw "fixture did not expose one unambiguous owned window" on main and every branch. Running a trimmed copy, bench/out/wt-pr32/tests/e2e/run-bench.sh (local, uncommitted): it pre-builds through serial_cargo, and skips the System Events window normalization, notifications.sh (posts and reads Notification Center = George's notifications) and os_surfaces.sh (may Dock-launch Notes, sets the volume). Log: bench/out/e2e-pr32.log. Pass → evidence for merging #32; fail → note the first failing fixture and move on.
- [x] Re-runs done (02:37), clean, n=3: Finder main 0/3, #24 old 2/3 (119.6, 34.5), #25 2/3 (135.1, 126.5). Seeded TextEdit: main 0/3, #26 old 0/3, #27 2/3 (90.4, 94.9).
- [ ] Two fixes win (#24 Finder, #27 TextEdit): after f24 + ts26, build the combined fork (main + #24 + #27 + #26 7cd59f4c) and run the full 5 tasks incl. textedit-seeded, n=3.
- Control 02:4x: merge order #24 → #6 (main merged in, CLEAN, CI) → retarget #27 to main, CI, merge → #26 (after ts26). Then the combined 5-task run ON MAIN; the hand-built wt-combo (cd5d2efe + df5d379e = e3bffd59, not built) only if a merge stalls.
- [x] Merged main 921d6d22 into #6 (clean, no conflicts) → pushed 9637e8b7; CI running.
- [ ] #6 CI on 9637e8b7: Test (macOS) failed in core `wait_event_tests::failed_inventory_poll_never_diffs_against_an_empty_baseline` (last_error Null vs APP_UNRESPONSIVE). It is a wall-clock race: a 20 ms timeout with no VirtualClock, so on a loaded runner the deadline passes before the first poll. #6 doesn't touch it (2 macOS files). Fix: branch test/wait-event-frozen-clock from main, which freezes the clock there and in retryable_seed_inventory_failure_is_preserved_in_timeout_evidence (same race). Tests running via serial_cargo (b2gnwc7ca). Then PR → merge → merge main into #6 again.
- [ ] FYI (control decided for the fork): find partial contract (#33). Panel walk ~230 elem/s, 3000 rows ≈ 15k elements ≈ 65 s, and Where: comes after the list in document order, so no complete+unique find can succeed. Proposal: find returns snapshot-style `complete:false, truncated:true` plus the matches seen, non-list content read first, refs only where the platform re-identifies by token. It drops uniqueness for partial answers. #33 holds an uncommitted query_stats.elapsed_us fix.
- Control decision for #33, additive, default contract unchanged: (1) find reads non-list content first and defers list/table rows; `--first` takes the first match in that order; measure with the 3000-file probe. (2) Opt-in `--allow-partial`: complete:false, truncated:true, matches seen so far, total_matches as a lower bound; refs only where the platform re-identifies by token; docs + skill updated. (3) One invert-verified test per point. Sent to #33.
- George's upstream short list: the find partial-results contract change is a candidate (fork only until he decides).
- [x] 03:25 STALL, found and fixed. My wait_event fix hung under its frozen clock: `WaitScheduler::wait` (crates/core/src/commands/wait_scheduler.rs:34) called std::thread::sleep, so virtual time never moved and retryable_seed_inventory_failure_is_preserved_in_timeout_evidence looped. The hung test held the build lock, and the f24 orchestrator, which holds the desktop lock, waits on the build lock: about 40 min lost (02:45-03:26). Killed it (sample in scratchpad/hang.txt); f24b resumed 03:26. Fix: the scheduler sleeps through crate::wait::deadline::sleep, which is std::thread::sleep outside tests. Re-test runs under `timeout 900`.
- Lesson: every serial_cargo test command gets a hard timeout, because a hung test holds the build lock and so stalls the desktop.
- [x] Control: no build-lock wait inside the desktop lock. orchestrate.sh now (a) requeues a round whose binaries are missing or fail check_binary_tree, with 3 passes 5 min apart and then SKIPPED, and (b) waits for builds to be idle BEFORE taking the desktop lock; if a build slips in, it releases and retries. serial_cargo caps: tests 1200 s, builds 3600 s. Pushed. The running ts26 orchestrator still uses the old code (replaced atomically, so its inode is untouched).
- [x] 03:27: the f24 orchestrator hit the 2 h background limit mid-f24b; its driver was killed and it left a stale lock under our shared owner name. Ran bench reset_desktop, removed 3 leftover bench t3-* folders, released the lock; ts26 started 03:29. f24b+f24c requeued as plan-pr24v2-rest.txt after ts26.
- Lesson: an orchestrator plan must fit the 2 h background limit. Split plans to 4 rounds or fewer.
- [x] 03:4x: serial_cargo jobs queued before 03:27 died with "line 32: syntax error near unexpected token `do'". My first cap edit rewrote serial_cargo.sh in place while waiting bash instances still had it open, so they resumed at shifted offsets. Later edits use an atomic replace. Affected: my wait-test job, and possibly the writers' queued jobs, which need relaunching. Rule: bench scripts that running processes read are only ever replaced atomically (write .new, then os.replace).
- [x] ts26 (orchestrator killed at its 2 h limit in ts26c's main entry): #26 7cd59f4c + #27: 3/3 PASS at 44.5, 34.7, 53.8 s (15, 11, 17 calls); main 0/2 at the cap. Seeded TextEdit is now under 60 s. Ran reset_desktop, removed leftover bench t4/t5 folders, released the stale lock; f24-rest took over.
- [ ] E2E #32 attempts: (1) my runner copy lacked the exec bit; (2) it stopped at the first gate with "reviewed binary version command failed". Root cause: the E2E Python helpers use `str | None` (needs 3.10+), and `python3` on PATH is Xcode's 3.9, so json_tool.py dies on import. This is probably also behind the browser's "fixture did not expose one unambiguous owned window" on every branch; not yet confirmed. Re-run 3 puts Python 3.11 (homebrew) first on PATH (bench/out/py311).
- Found (repo, upstream-worthy): tests/e2e needs Python ≥3.10 but checks no version; on stock macOS python3 (3.9) it fails with a misleading gate message.
- [ ] PR #49 (test/wait-event-frozen-clock, 7cfd7eeb): freezes the clock in 2 wait_event tests, and the scheduler now sleeps through deadline::sleep. 289 wait tests pass, clippy and the rules are clean; invert-verified (hangs without the scheduler change). Watching CI → merge under the rule → merge main into #6 again. Queue: #24 → #49 → #6 → #27 → #26.
- Lesson: `timeout 90 cargo test` inside serial_cargo did not stop a hung test binary. Only serial_cargo's own cap did, at 1200 s.
- [ ] PR #48 skill recipes (4fbb4e43, on #24 v2): batch refs can't come from earlier entries (src/batch/preparation.rs:18-57). 5 tests check every shipped batch example and the synopsis. Text skimmed: app-agnostic. Building wt-pr48, then plan-pr48.txt (s48a-c: #48 vs #24v2 vs main, rotated) after f24-rest.
- [ ] #12 (fix/e2e-prerequisites: python ≥3.10 check up front, empty arrays on bash 3.2, which also fixes the `cleanup_files[@]: unbound variable` I hit). Merged main (21 behind, clean), pushed; CI watched. Merges after #49. Queue: #24 → #49 → #12 → #6 → #27 → #26.
- [x] E2E #32 run 3 (Python 3.11, trimmed: no System Events/notifications/os_surfaces): it got past the fixture gate ("fixture launched in background with exact PID, generation, and window ownership" PASS). So the "unambiguous owned window" failure was the Python 3.9 import crash, not Screen Recording. Totals: 125 PASS, 9 FAIL lines (6 distinct + summary), 1 SKIP. Marked contaminated: "fixture ownership changed; expected pid 90292, found none". FIRST FAILING: "[headed] click dispatches exactly once" (ok=False, no mechanism). Others: STALE_REF case got APP_UNRESPONSIVE; wait actionable click; headed drag; trace screenshots pngs=0 (Screen Recording denied, expected). Not compared with main; moving on as ruled. Log: bench/out/e2e-pr32-run3.log.
- [ ] #32: control says merge under the rule once the probe's `snapshot --window-id` case passes again with a 0700 state dir on 25cbc103. Queued pr32-r5 (own-tree binary from bench/out/wt-pr32; the probe makes ROOT and STATE at 0700). Pass → merge after #26. Fail → stays a draft, cause written down. The contaminated E2E counts neither for nor against it.
- UNVERIFIED, nobody on it tonight (E2E #32 run 3, no comparison with main): `[headed] click dispatches exactly once` (ok=False, no mechanism); headed drag delivers no gesture; "removed target is refused as STALE_REF" got APP_UNRESPONSIVE (not_delivered); wait actionable click. Log: bench/out/e2e-pr32-run3.log.
- [x] #32 probe r5 on own-tree 25cbc103 with STATE at 0700: PASS on all 12 checks, including both `snapshot --window-id ... data.window.title` cases (Screen Recording denied). Merge-ready under the rule; it merges after #26.
- [ ] PR #50 (cut 2, fix/sheet-confirm-effect a95bce25, stacked on #24): return/enter/escape on a sheet or dialog are verified by effect (the sheet closes or the window set changes), then a real-key fallback, else ACTION_FAILED retry:unsafe. Finding: on main, `press return` on Go to Folder only AXConfirms the path field (ends editing), never sends a key, and the sheet has no default button. Probe read and queued (bench-only, own-tree binary): n=3 head vs main plus plain-escape timing. Still open: perf A/B for the extra AX reads on every press. Conflict with #26 expected in key_dispatch.rs.
- [ ] #25 head 21680b2b: no CI because the PR is DIRTY (conflicts with main; pull_request CI doesn't run on unmergeable PRs). Asked the writer to merge origin/main, run the gates via serial_cargo, push, and repin pr25-v4.sh. Its v4 probe adds raise_bench for A2 and prints the full Cancel-by-ref error. Cancel-by-ref is accepted as not #25's. Labels are now `File > item 7`.
- [x] f24 done (#24 v2 df5d379e vs main ec3add89, n=3): #24 3/3 PASS at 96.5, 51.0, 81.5 s (median 81.5; 25, 22, 25 calls). Main 1/3 (99.8 s), 2 cap fails.
- [ ] #24 merge: main 47194fa6 merged in cleanly (11 behind), pushed; CI watched → squash-merge under the rule. Overlap is skills/docs only (#25, #26, #32, #36, #41), no code. After merge: retarget #48 and #50 (stacked) to main.
- [ ] #50 probe FAIL (a95bce25): head goto-return 2/3 `delivered_verified` effect=observed after AXConfirm while the sheet stayed open and nothing navigated (a false VERIFIED). 1/3 honest ACTION_FAILED, but the real-key fallback also did not navigate. Base: 3/3 delivered_unverified, false success. Plain escape: no added cost (head 173-194 ms vs base 175-220 ms). Sent back to the writer: find which signal fired, require a stable absence of the sheet, check where the fallback key lands. Stays a draft.
- [ ] #49: 13/13 green on 7cfd7eeb, but 4 behind main. Merged main → c12b50fd; CI running again. Main already has #45 (24574e93), which froze one of the two tests, so #49 now adds only the retryable_seed freeze and the scheduler sleep. PR body updated. Merge order stays #24 → #49 → #12 → #6 → #27 → #26 → #32, with main merged in again before each.

- [ ] #25 v4 (d5e01b9d, 04:19): everything passes except A3: `--headed` press from the background returns AMBIGUOUS_TARGET, no folder made. The probe printed only the code. Sent back for the cause and a repin with the full error. The probe's cancel jq has a syntax error (probe bug, not #25).
- [x] #24 squash-merged 04:3x PST → main 7ba6b7d0 (13/13 on b5e1f370, CLEAN, main in). #48/#50 retargeted to main; both now DIRTY from the squash. #50 writer (debbcab2) told to merge main. #48: merge main into wt-pr48 myself after the s48 rounds (its binary is in use).
- [ ] #25 v4 A3 AMBIGUOUS_TARGET: the writer says it's the existing headed window lookup (window_lookup.rs:81) with two visible Finder windows and none focused. Rerun with redacted window titles is queued (bclhfs9cd).
- [x] #25 v4 rerun: A3 AMBIGUOUS_TARGET confirmed as clutter (7 candidates: 5 leftover bench-root windows, 1 personal, 1 untitled), accepted as not #25's. Everything else passes. #25 is done from my side once CI is green.
- [x] PRIVACY: a Finder window on a personal folder (title matched the home folder or a sidebar name) was open during the s48 rounds. Closed it by title without reading it. Scan of the s48 run logs (counts only): no home-only rows ("Public" = 0 everywhere), no snapshot whose window title is personal. Source unknown, likely a Finder reopen or a run's new window. Fix 50af8d06: reset_desktop now closes personal-folder Finder windows by title.
- [x] #25 writer done: 13/13 on d5e01b9d, CLEAN, draft, PR body has v4 results. Not in control's merge queue, so it stays a draft until control decides; listed in the final CHECKPOINT.
- [x] s48 rounds done (04:13-04:37, Finder, Opus 5.5, 150 s cap, n=3 per arm):
  - #48 skills (4fbb4e43): 2/3, 93.7 and 125.9 s, one timeout; 24, 23 and 27 calls.
  - #24 alone (df5d379e): 2/3, 59.1 and 50.2 s, one timeout; 15, 19 and 26 calls.
  - main: 0/3 (104 s fail, two timeouts); 32, 18 and 35 calls.
  - Verdict: the recipes don't beat #24 alone at n=3; they're worse on wall time and no better on calls. All of s48c failed, including #24. s48c's main run (bcb5) started 04:29:03, while I closed the personal Finder window at about 04:29:30, so bcb5 is suspect.
- [ ] #48: merging main (one conflict in the macOS SKILL paragraph; kept #48's version, main's plus the recipe sentences). The commit hook ran cargo outside serial_cargo; I stopped it and am rerunning the hook through serial_cargo (bk1hbcqrn), then committing with SKIP_PRECOMMIT and pushing.
- Lesson: a merge commit in a worktree with core.hooksPath runs the pre-commit cargo gates. Wrap it: `serial_cargo.sh bash .githooks/pre-commit`, then `SKIP_PRECOMMIT=1 git commit`.
- Control 04:4x: #48 stays a draft, not merged; its body now has the s48 table. #25 merges under the rule after #32. #12 now names the python3 it found and gives a copy-paste fix (5eb71bfb, main merged in as 463ea38b, pushed); its body is updated.
- [ ] A3 headed-lookup clutter (own item, not #25): a `--headed --app` press fails AMBIGUOUS_TARGET when the app has 2+ visible windows and none is focused (window_lookup.rs:81). Seen with 7 Finder candidates. It is the existing contract; the suggestion names --window-id, but with Screen Recording denied list-windows has no titles to choose by. Possible follow-up: prefer the main or front window for --headed when exactly one is AXMain.
- [x] Personal Finder window, cause: the most likely source is the agent's bare `agent-desktop launch Finder --activate` (no path). A scan of all run logs (command lines only) found 32 bare `launch Finder` calls across 27 runs, including s48 runs 6731, 6b5f and bcb5. launch uses NSWorkspace openApplicationAtURL, which sends a running Finder a reopen, and Finder opens its default location (Home) when it has no window open. The bench scripts never launch Finder without a path: setup runs `open <bench dir>` and macctl activate/raise. Only cli_latency.py runs `launch Finder --activate`, and it wasn't run tonight. Not confirmed live: confirming means opening Home, which the privacy rule forbids.
  - Guard f63f865c: setup requires the bench window before the agent starts, cleanup and reset close personal windows by title, and closes are logged as counts in bench/out/personal-window-closes.log (1 so far, manual, at 04:29).
- [ ] TOOL item (control 04:5x): `launch <app> --activate` on an app that is already running sends a reopen (NSWorkspace openApplicationAtURL, crates/macos/src/system/launch_bridge.m:163). Any app with no window then opens a default window the agent never asked for (Home in Finder, a new document elsewhere).
  - Candidate fix, app-agnostic: when the app is running, `launch` activates it through NSRunningApplication with no reopen. A reopen is sent only when an explicit option asks for a new or default window. The result reports which path ran.
  - Probe first, bench-safe apps only: TextEdit's untitled doc, the fixture app. Never confirm the Finder case live.
  - Writer queued after #50's revision; Sonnet if it stays small.
- [x] #50 rework a0383afa: probe v2 running under the lock (bpytu3xu2). The binary came from t3-c6d51644 at a0383afa; the probe touches bench folders only.
- [ ] Launch-reopen writer started (Sonnet, fix/launch-running-no-reopen, task b8a36fc5). Probe-first on TextEdit and the fixture app only, never Finder.
- [ ] #50 v2 probe FAIL (a0383afa, 04:51):
  - head goto-return 3/3 is a false delivered_verified with signal surface_closed, while the sheet stayed open and didn't navigate. Focus after the press is AXSheet, and the fallback key never ran.
  - Variants: key-only navigates 3/3, confirm-key 0/3, confirm-refocus-key 3/3.
  - Plain escape adds no visible cost.
  - Sent back: real key first, and fix the false closed signal.
- [ ] A personal Finder window was closed AFTER the #50 probe (count 1, logged). Source unknown: the probe opens bench paths only, then uses Go to Folder, AXConfirm, Return and escape. Watch the next close counts.
- [x] Lingering windows found and fixed (05:0x):
  - Finder had 8 bench windows plus 1 personal-folder window, each with a Go to Folder sheet open (probe leftovers). A window with a sheet ignores AX close, so every earlier close, including my 04:29 one, failed silently and the log counted them anyway.
  - Cleared under the lock: raise, Escape, close. 0 personal and 0 bench windows left.
  - TextEdit had a bench document with an expanded Save sheet. Cancelled it via AX, closed the document, quit TextEdit. That window is why #33's probe aborted ("seed window did not open").
  - Fix 22b8eb8b: close_finder_windows raises, sends Escape, closes and verifies, and the log records verified closes plus what was left.
  - Likely origin of the personal window: a #50 probe run where Return landed in the Go to Folder suggestion table and navigated to a suggested folder. The window had a Go to Folder sheet like the others. Not confirmed.
  - Real count for control: 1 personal window, closed once, verified, at 05:0x.
- Control 05:1x:
  1. #50 v3 is its last try: real key first on sheets, no surface_closed. If v3 shows any false verified or navigates fewer than 3/3, park it here with the v2 variant table (key-only 3/3, confirm-key 0/3, confirm-refocus-key 3/3) and move the writer on. Writer told.
  2. Go to Folder privacy rules are in bench/README.md and bench/BRIEF_RULES.md (7f538d95). Every brief copies BRIEF_RULES.md.
  3. Window hygiene: bench/probe_slot.sh plus post_probe_check.py fail any probe that leaves windows or sheets, and close the leftovers. Use probe_slot.sh for every probe from now on.
- [x] #49 squash-merged → main d0091626 (13/13 on c624b349, CLEAN). Main merged into #12 (89046f87) and #6 (9d13277e) and pushed; their CI is running.
- [x] #33 probe on 2b97fa84 (n=3, clean desktop), expanded 3000-file Save panel:
  - `--first` Where:/Cancel: head 246-285 ms with a ref, against base TIMEOUT at 5 s (and at 20 s).
  - Plain find: still TIMEOUT, but at 4.07 s on head against 5.06 s on base.
  - `--allow-partial`: truncated answers come back, but with refs 0 even for buttons.
  - Collapsed panel: no regression.
  - Asked the writer about the 4 s budget and the missing refs.
- Control: #33 merges on the fork after #25, no hold for George. Conditions:
  1. The default find budget, about 1 s shorter, gets restored (control's preference) or is explained in the PR body.
  2. `--allow-partial` refs 0 gets fixed (token refs on macOS) with an invert-verified test, or is shown from the code and the docs corrected.
  Then CI green and CLEAN. Merge queue: #12 → #6 → #27 → #26 → #32 → #25 → #33.
- Control accepted both #33 answers: keep the 4.07 s timeout path, with the reasoning in the PR body. #33 merges after #25 once the live rerun shows refs on partial matches and CI is green.
- [ ] #12 CI on 89046f87: Test (Windows) failed in `an_open_shell_surface_resolves_through_the_adapter_seam_and_roots_a_tree` with a UIA_E_TIMEOUT reading the desktop root. #12 only touches tests/e2e shell scripts, so the failure is unrelated (a Windows UIA timeout). Main moved again (#47 → 0a5ff966, merged by another session), so main is merged into #12 (26c0df49) and #6 (61e2b358) and CI reruns. If that Windows test fails twice on unrelated heads, file it as a flaky test.
- [x] #50 v3 PASS (bc1dba84, 05:15, through probe_slot.sh, hygiene clean):
  - Head goto-return 3/3: real key, delivered_verified on surface_window_closed, and every run independently closed the sheet and navigated to DEST. No false verified.
  - Base 3/3: AXConfirm with no effect (false success).
  - Sheet path costs 514-590 ms against base's 130 ms, from the two-read postcondition; the writer will explain it in the PR body.
  - Plain escape: no cost.
  - Proposed to control for the merge queue. It still needs the perf A/B (scripts/perf-baseline-compare.sh).
- Control: #50 approved for the queue after #33, once the perf A/B runs (b09qpid9q: base = 7ba6b7d0 tree via wt-pr24m, fixture A/B under the lock, builds through serial_cargo) and the PR body explains the 514-590 ms sheet-path cost. #12 Windows UIA_E_TIMEOUT: covered by the full CI rerun on new head 26c0df49. If the same test fails again on any PR, open a small flake-test PR and don't hold #12.
- [ ] Row-naming writer started (Opus high, fix/row-name-from-text-field, task ba6497ea). Derived names may be used for locating and display, never in the capture proof.
- [x] #50 perf A/B (bc1dba84 vs 7ba6b7d0, fixture, 10 rounds): no regression; every case within noise (snapshot ~237 ms, get 47, click 113-115, set-value 62-64, type 69). Sent to the writer for the PR body.
- Control on #54 (launch-reopen, d883c770):
  - Option (a): no TextEdit in the probe (its Open panel can show iCloud or Documents). Add a second real app: Chrome on a throwaway --user-data-dir under ~/agent-desktop-bench, no CDP, never the real profile. Take it under the desktop lock and tell the browser lane before it runs.
  - Before merge: fix the 8-field LaunchResult (7 is the cap), and the skill must say `--activate` on a running app no longer opens a window and when to use `--reopen`.
  - Queue position: after #50. Writer told.
- Fixture-only launch probe running (bz4ztaniw). #33 fb29e173 probe queued (pr33-v4) behind it.
- MERGE RULE (control, all lanes): "main merged in" is met when the PR is CLEAN, CI is green on a head with a recent main merge, and there's no file overlap with the commits that landed on main since that merge.
  - Always overlap, whichever side touches them: Cargo.toml and Cargo.lock, .github/**, crates/core/src/lib.rs, AGENTS.md, any skill file both sides touch, and command-surface or count files when either side adds a command.
  - Main red after a no-remerge merge: a revert PR merged under the rule, never a direct push. Watch main CI after every merge.
- #12 and #6: #51 touched core lib.rs, so main 3fce1ebb is merged in again and CI reruns. Control asked whether "always" means one side or both sides.
- [x] #33 fb29e173 probe PASS (05:29, hygiene clean): partial refs now present (cancel 1/1, buttons 20/20); --first 255-278 ms with a ref against base TIMEOUT; plain find TIMEOUT at 4.07 s as intended; collapsed unchanged. Control's conditions met. #33 merges after #25 once CI on 669c9b37 is green.
- [ ] #54 fixture-only probe INVALID: every launch exited 1 on both binaries, and the error wasn't recorded. A diagnostic with the fixture not running: head ok. Sent back to record errors and to add the Chrome case.
- [ ] Queue prep 05:4x:
  - Main 3fce1ebb merged into #26 (9a9820f8), #32 (fde8cf74) and #25 (243b153a) in bench worktrees wt-m26/32/25; no conflicts; pushed, CI running.
  - #6 red on 6f4306da: Test (Windows) `slow_pre_capture_preserves_dispatch_budget` TIMEOUT. That's a core trace timing test; #6 changes macOS element_bounds only. First occurrence; the failed job reruns once the workflow ends.
  - #12 CI still running on 40d0ae37.
- [x] #56 row naming (0a828629, draft): probe sent back. Drop the TextEdit Open panel (can show iCloud or Documents) and point_save_folder (the pref write is blocked here). Second app: Finder list view of a bench window.
  - Probe PASS (2 clean runs, 0 left): find --name <file stem> on a file row. TextEdit expanded Save sheet head 3/3 with ref (role cell), base 0/3; Finder list view head 3/3 with ref, base 0/3. Latency equal: head 325-373 ms, base 323-371 ms.
  - Probe bugs fixed by me (first failure each): AXFocusedWindow returns the sheet itself, so panelax found no sheet and the cleanup Escape was skipped; agent-desktop titles a Finder window "Finder" (app name, no Screen Recording), so the window id now comes from list-windows (exactly one window, focused, AX says the focused window is the bench folder). Escape does not dismiss an expanded Save sheet: cleanup presses Cancel.
  - Bench (05867170): probe_slot.sh creates its output dir (a missing dir skipped probe and hygiene check); post_probe_check counts seed-* TextEdit docs and presses Cancel on leftover sheets (bench/sheetctl.swift).
  - [ ] perf A/B vs 3fce1ebb (bench/out/probes/perf56.sh), then PR body, then queue after #54.
  - [ ] Control: before queueing, cite the test that keeps the derived name out of the capture proof, and invert-verify it. Tests: refs::ref_capture_proof::derived_name_tests (a_derived_name_is_displayed_and_located_but_never_recorded_as_identity, the_capture_proof_never_sees_a_derived_name), guarding identity_name() in ref_alloc.rs. Invert run in bench/out/wt-inv56. Then queue after #54.
- #52 (9ea5d145) landed on main and touches AGENTS.md (always overlap): main merged into all seven queued PRs at 06:0x: #12 6e3cfd6f, #6 97593870, #26 2bccc5aa, #32 3a020d4b, #25 7141e0e0, #33 0f4c8c15, #50 82cd977f. Clean merges (docs/json-output.md and contract_command_surface_tests.rs auto-merged). CI running. The old #6 flake rerun was dropped; the new run replaces it.
- #54 is DIRTY (needs main); its writer owns that.

- #56: identity test cited and invert-verified. Breaking identity_name fails both derived_name_tests (lines 105 and 133); restoring it passes them. Perf A/B vs 3fce1ebb: within 4 ms on all 8 cases, no regression. PR body updated. Main merged in (bd6327e1). Queued after #54.
- #54 probe 0a5e3b4e PASS on Chrome (throwaway profile, browser lane told first):
  - base --activate opened a new window 3/3;
  - head --activate 0/3, 122-125 ms;
  - head --reopen window 3/3, 246-292 ms.
  The fixture's APP_NOT_FOUND on base and --reopen is a Launch Services lookup of an unregistered copy; base fails the same way. This was the earlier "exit 1".
- [ ] #54: main merge conflicts with main's LaunchCdpArgs/--download-dir. After the merge, LaunchOptions has 8 fields and CLI LaunchArgs 9 (main already has 8). Sent to a Sonnet writer: merge main, group activate/reopen to stay at 7 or fewer, gates, PR body with the probe table. Then the perf A/B, and a check of the release binary.
  - Control: #56 and #54 accepted. The #54 writer also brings CLI LaunchArgs to 7 or fewer, fixing main's 8 from #47. CDP/download args stay in LaunchCdpArgs. The PR body names the #47 breach. Writer steered.
- #54 50d4f855 (writer: LaunchPresentation and LaunchAttachArgs, every struct at 7 or fewer). Release re-check PASS on Chrome: head --activate 0/3 at 115-141 ms, --reopen 3/3, base 3/3. Perf A/B vs 9ea5d145: within 5.3 ms, no regression. Body updated, with the cause sentence softened (no personal-folder claim). Waiting on CI, then the queue after #50; #56 after #54.
- MERGED #12 → main 6d7833b6, then #6 → 45236e6a (no overlap: #12 touched only tests/e2e). Main CI on 6d7833b6 was cancelled (superseded); 45236e6a is running and watched by bj58prm42.
- #27 retargeted to main. Main merged in (af287031): element_bounds conflicts from #6's squash, resolved to #27's side because main's clip_by_ancestors equals #27's pre-change version. element_bounds 18/18 and hit_test 11/11 locally. CI watched. Mark ready before merge.
- MERGED #27 → 4eb2e47d, #26 → 23eb4751. Main CI: every run before 23eb4751 was cancelled by the next push; 23eb4751 running.
- Control narrowed the rule for this queue: skills-only overlap merges without a remerge (merge-tree clean, CI green on a main merge-in under 3 h old, no skill-content tests on main's side). Code overlap still remerges. I treat docs/json-output.md as non-code too (flagged to control).
- #32 remerged c603ba3c (mod.rs list). #25 remerged 8d9c9c47: real key_dispatch conflict, #26 hosted focus vs #25 menu_shortcut; combined, macOS lib 901/901, clippy and fmt clean. #50 remerges after #32 (shares cg_window.rs). #33, #54, #56 doc/skill-only.
- Control: docs/json-output.md is treated like the skills, except that src/cli/doc_literal_drift_tests.rs reads it (include_str!), so a PR that edits it remerges whenever main changed it. Order: #32 → #25 → #50 → #33 → #54 → #56. #50, #33 and #56 each remerge right before their turn (each one's merge changes json-output.md for the next). #25 and #54 edit neither json-output nor the drift-read files.
- Control reorder: #32 → #25 → #54 → #50 → #33 → #56. Name-only check: #54 shares code with #25 (crates/macos/src/system/mod.rs) and #33 (src/cli_args/mod.rs), and only skills with #50 and #56. #50 shares key_dispatch.rs with #25. Plan: after #25 merges, remerge #54 and #50 together (parallel CI), merge #54 then #50, then remerge and merge #33, then #56.
- Main CI GREEN on 23eb4751 (covers #12, #6, #57, #27, #26).
- MERGED #32 → 8e8b588a, #25 → 950abe84 (skills-only, merge-tree clean). Main CI 950abe84 watched (bfobj1307).
- #54 remerged 30742f51 (clean), CI running.
- [ ] #50 remerge is a 3-way design conflict in key_dispatch press_for_app: #50 modal route vs #26 hosted focus vs #25 menu_shortcut, plus json-output. Delegated to an Opus writer (worktree), which re-pins the sheetconfirm probe. Proposed to control: #54 → #33 → #56 → #50 last. Holding #33 until control answers.
- Control: order #54 → #33 → #56 → #50 last. #50 brief steered: live probe on a hosted Save panel (#26) and Go to Folder (#50), n=3 each, via probe_slot, no regression against either's earlier 3/3.
- #33 remerged 970ca1f7 (clean), CI running in parallel with #54.
- Main CI GREEN on 950abe84 (covers #32 and #25).
- #50 merged-main head 9c34a925 (Opus writer: the hosted route wins inside a sheet; one KeyTarget delivery pid; finish_press merges details). Live, both PASS and clean, via probe_slot:
  - Go to Folder: head 3/3 delivered_verified with surface_window_closed, 540-552 ms (1005 cold); base 0/3.
  - Hosted Save panel: head and base 3/3 saved, 12/12 ok, 6/6 hosted_view observed, 0 modal_key.
  - I patched both probes to set the path through macctl set-focused instead of find in the sheet (suggestion-table rule).
  - PR body updated. Still needed: a final main merge-in at the end of the queue, and CI.
- #54 30742f51: Test (Windows) x64 hung 43 min in the lib-test step (output captured, no test name) and was cancelled at the 45 min timeout. ARM64 passed the same tests in 7.5 min. The x64 job also runs live WPF tests. Rerun once (flake rule). If it hangs again: a small test PR so the step streams per-test output and a per-test timeout names the test; don't hold #54.
- #54: e50786e2 (FFI reentrancy, another lane) touched crates/ffi/src/apps/launch.rs. Remerged → 4e5e0a5f; locally FFI builds and tests pass. CI again (~45 min).
- Proposed to control: merge #33 (green, no overlap) before #54, applying the same cli_args/mod.rs exception in reverse. Waiting for the answer.
- MERGED #33 → d5b6e7ac (no overlap).
- #54 4e5e0a5f qualifies under control's exception vs d5b6e7ac: merge-tree clean; the cli_args/mod.rs hunks are only #33's allow_partial field and #54's mod line; otherwise skill only; #54 doesn't edit json-output. Merges when its CI is green.
- #56 remerged 5f7a31d6: json-output and commands-observation conflicts with #33's find docs, both kept. Drift 3/3, skills 14/14, core lib 1831/1831. CI running.
- Control: if the Windows lib-test hang happens a second time on any PR, open a flake PR that runs lib tests with --nocapture or a per-test timeout so the test gets a name.
- FLAKE, second occurrence: core trace::trace_artifacts::dispatch_budget_tests::slow_pre_capture_preserves_dispatch_budget failed on Windows x64 on #54 4e5e0a5f (first was #6 6f4306da). The same run also failed artifact_capture_budget_supports_deadline_bound_screenshot_backends (left 1, right 2). Per the rule: Sonnet writer on test/trace-budget-windows-flake, draft PR; #54 not held, failed job rerun once after the run completes (by8lw0h5v).
- Main CI GREEN on d5b6e7ac (covers e50786e2 and #33).
- [ ] #59 (flake PR, head e972d52d): the two Windows trace timing tests moved onto VirtualClock. Test only, reviewed. Rerun Test (Windows) a few times when CI is green, then queue it after #50.
- [ ] After #50 merges: CHECKPOINT, then wait up to 30 min for a T3 restart (TextEdit permission). On restart: run check_save_folder.py once; if the write works, include the stock textedit task. With no restart by then: run with textedit-seeded, labeled not Aside-comparable. n=3, every binary through check_binary_tree, plans under 2 h.
- [x] #54 → 282616ff, #56 → a48763f5, #50 → d2e406cb merged. Main CI green on a48763f5; d2e406cb running.
- [ ] Final run prep: wt-final at main d2e406cb (release build running). Plans: bench/plan-final-textedit.txt (Aside-comparable) and bench/plan-final-textedit-seeded.txt (not comparable). 3 rounds × 5 tasks, one arm (main).
- Found: bench/cargo_env.sh resolves serial_cargo.sh through BASH_SOURCE, so sourcing it from zsh fails. Run it under `bash -c`.
- [x] 16:47Z T3 restart: background jobs lost (#59 reruns, main CI watch, wt-final build) and restarted. Lock free, desktop clean, bench reset run.
- [x] check_save_folder.py once: FAIL. The pref write works and is restored, but the Save panel's Where: is still not the bench folder. Stock textedit stays blocked.
- [ ] Final run started 16:50Z detached (bench/out/start-final.sh → orchestrate, log bench/out/final-run.log): plan-final-textedit-seeded.txt, NOT Aside-comparable. CHECKPOINT per round to control v2 (mcp:1f6d7f49-...).
- Control thread moved to mcp:1f6d7f49-e90f-4bb7-8055-7282273fec73.
- [ ] Control: George accepted textedit-seeded for the final run. Label it "TextEdit seeded, not Aside-comparable" in RESULTS.md. After the run, add the Save panel default-folder problem as its own TASKS.md item.
- Found: main CI on d2e406cb went red on Test (Linux) with `snapshot_completion_log_omits_private_app_and_window_names` (snapshot_log_tests.rs:68, 0 events captured, expected 1). #50 touches no core code, and the test passed on a48763f5. It looks like the tracing callsite-interest race (a scoped `with_default` subscriber in parallel tests). First occurrence; job rerun once. A second failure on any PR → test PR.
- Control: if the Linux rerun fails the same test again → a small test PR for the tracing callsite race, not a revert. Hold #59 until main is green. Each round CHECKPOINT carries per-task token totals (goal: under ~1M total).
- [x] Main CI green on d2e406cb after one Linux rerun (snapshot log test: first occurrence, a flake).

## PARKED 2026-11-08 ~17:15Z (Mac traveling)

Final run on main d2e406cb (n=3, textedit-seeded): medians weather 21.9 s, settings 26.8 s, finder 42.5 s (17 calls), textedit-seeded 75.5 s (17 calls, 2/3 ok; fin3 timed out at 150 s after 22 calls), calculator 17.4 s. Written into bench/RESULTS.md.

Next plan
- [ ] Cut Finder and TextEdit calls: Finder median 17 calls, TextEdit 17. Target under 60 s and under ~1M tokens per round.
- [ ] Fix the stock TextEdit Save panel folder (its own item): check_save_folder.py writes the pref and restores it, but the Save panel's Where: is still not the bench folder, so the stock `textedit` (Aside-comparable) task cannot run.
- [ ] PRIVACY (needs George): the Save panel and Go to Folder sheet snapshots hold non-bench row names (sidebar, browser and suggestions). 47 runs tonight captured them: textedit-seeded runs (up to about 3,000 named rows) and finder runs (1-15 rows, likely the Go to Folder suggestions). I deleted those runs' ad-home snapshots and stream.jsonl, and stripped the answer, steps, cli and verify text from their summary.json and results.jsonl rows. Only counts were read, never names. These listings were already sent to the model during the runs. The textedit-seeded task needs a change so the panel never shows a non-bench location, or the task is retired, before another run.
- Stop rule: two fix rounds in a row with no lower median → stop and report.
- [ ] #59: merge when the 3rd Windows rerun has passed and it's CLEAN (main green on d2e406cb).
- [ ] F9 (fix/window-title-from-ax): writer told to commit WIP and push; gates unfinished.
- Found: macctl close-discard didn't press TextEdit's "Discard" (Duplicate/Discard/Cancel sheet), which left a seed doc behind after fin3. Fixed in bench/macctl.swift.
- [x] F9 pushed: fix/window-title-from-ax 2e5dd2f2. The hook and full cargo test pass (3,089). No PR, no fork CI.
- [ ] F9 decision (needs control/George): main's #32 already fixes this bug with a different rule (`""` when the title can't be proven; F9 falls back to the app name). They conflict in 8 files under crates/macos/src/system/window/. Recommendation: keep #32 and drop F9, unless a gap in #32 shows up in a probe; then port only that piece onto main.
- [ ] Final 1.3.0 A/B (control 2026-11-08): upstream b43ac4f6 vs origin/main at start, n=8, plan bench/plan-final-ab.txt. Detached waiter (scratchpad final_ab.sh, log bench/out/logs/final-ab.log) starts at 22:15 PST, or earlier on `touch bench/out/final-ab.go` once #76/#38 settle. Then rewrite the #62 macOS Performance line from that run only: one clause per lever, keep the drift caveat.
- Found (2026-11-08 19:0x PST): #62 4f17c090 dry-run Linux jobs failed on apt (archive.ubuntu.com unreachable on the runner); consumer and archive-contents jobs failed downstream. Failed jobs rerun.
- Found (2026-11-08 19:25 PST, main 61579175, 3/3): headless `right-click` on AgentDeskFixture's text field returns ACTION_FAILED delivered_unverified "All chain steps exhausted" while the context menu opens (independent: fixture-pid popup window). #36 (387dd2bd) gives the same; it only settles AXShowMenu -25204. #62 Known issue added. Post-release: settle the right-click chain by reading the menu surface on every failing step, not only -25204. Probe: ~/agent-desktop-bench/probe-rightclick/probe.py.
- [x] #76 merged 61579175 (perf flat, probe 0/36 vs a0d644b4 1/6). #38 merged cf03b9d3 (main merged in, perf flat). Final A/B triggered 19:29 on cf03b9d3.
