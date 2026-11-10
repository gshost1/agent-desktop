# TASKS: macOS speed track, agent-desktop 1.4

Mac (M2, 8 GB), Opus 5.5, one session, no subagents. Bench harness: `~/.t3/worktrees/agent-desktop/perf-speed-cost/bench` (unchanged).

## Fixed 1.4 protocol

- Tasks: `weather`, `settings`, `calculator` from `bench/tasks.py` (prompts and independent checks unchanged). Finder and TextEdit stay retired.
- Driver: `bench/driver.py`, config `a` (shipped `skills/agent-desktop/SKILL.md` of the build's own tree appended), `claude -p`, Bash only, model `claude-opus-5-5`, default effort.
- Builds: A = tag `v1.3.0` (930cb49a), tree `~/.t3/worktrees/agent-desktop/bench-v130`; B = the candidate tree (fix branch or main). Each tree's own release binary and skill.
- Runs: 8 per task per build (48), 8 rounds, one round per desktop-lock turn, task order and A/B order rotated per round exactly as `bench/plan-final-ab.txt`, all rounds in one paired session through `bench/orchestrate.sh`.
- Pass: the driver's `success` (independent observation), rows with `valid != false`.
- Time: per-task median of `wall_s`, summed over the 3 tasks; mean sum beside it.
- Tokens: per run `input_tokens + cache_creation_input_tokens + cache_read_input_tokens` from `result_usage`; per-task median, summed.
- Turns: per run `api_calls`; per-task median, summed.
- Reader: `ab_summary.py <results.jsonl> <since run_id> <treeA> <treeB>` (scratchpad copy; reproduces the 1.3.0 figures 52.0/43.2 s, 174K/118K, 12/9 turns from rows `110819*`).
- Target: B beats A in the same paired session, and B's sums are under 43.2 s and 118K, with 24/24.
- Fix-level checks before any model run: model-free direct timing, min of 7 with warm-up discarded, median and max beside it.

## Phase 1: where time and tokens go (1.3.0 paired A/B, main cf03b9d3, 24 runs)

| Task | Wall | Model | CLI | Turns | Input tok | Prefix/call | Biggest CLI items (median) |
|---|---|---|---|---|---|---|---|
| Weather | 13.3 | 6.7 | 6.0 | 2 | 27.8K | 12.2K | launch 2.94 s, first snapshot 3.07 s |
| Settings | 16.4 | 8.7 | 6.7 | 4 | 50.6K | 12.2K | first `find` 3.11 s, launch 1.41 s, click -w 0.5-0.95 s |
| Calculator | 13.5 | 8.6 | 4.6 | 3 | 39.3K | 12.2K | first snapshot ~2.3 s, launch 1.19 s, 11 clicks x 75 ms |
| Sum | 43.2 | 24.0 | 17.3 | 9 | 117.6K | | |

- Turns are at or near the floor (look, act+read, answer). Tokens are ~93% the 12.2K per-call prefix (harness ~9.9K, core skill ~2.3K).
- CLI is 40% of wall; ~6 s of it is the 2 s renderer settle on each app's first observation (PR #5, other worker).

## Ranked fixes (expected saving on the 3-task suite)

| # | Fix | Kind | Expected | Risk | Files | Owner/state |
|---|---|---|---|---|---|---|
| 0 | 2 s settle only for Chromium | product | -6 s (first obs 3.0 -> 1.0 s Weather, 2.35 -> 0.30 Calc, 3.4 -> 1.0 Settings, measured here on bf14dac8) | low | renderer_activation.rs | PR #5, other worker |
| 1 | Skill: walk a path the task names in one call (find + click -w per step, read at the end) | skill | Settings 4 -> 2-3 turns: -3 to -6 s, -13K to -26K tokens | medium (blind chains) | skills/agent-desktop/SKILL.md | mine, first |
| 2 | Skill trim: move OS-specific identity and PowerShell text out of the core skill | skill | ~-0.5K tokens per call, ~-5K suite | medium (guidance loss) | skills/agent-desktop/SKILL.md, platform skills | mine |
| 3 | launch: cap the window poll interval at 50 ms (now grows to 250 ms; launch returns 0.05-0.55 s after the CG window shows) | product | -0.1 to -0.4 s per launch | low | crates/macos/src/system/launch.rs | mine |
| 4 | Native first observation: skip the second full walk after AXEnhancedUserInterface is set when the app is not Chromium | product | Weather -0.28 s, Settings -0.4 s, Calc -0.1 s | medium | renderer path (after #5 merges) | mine, waits on #5 |
| 5 | WINDOW_NOT_FOUND after a plain launch while a full-screen app holds the screen: say the window is on another Space and suggest `launch --activate` | product | bench-neutral; saves exploration turns for agents in a full-screen terminal | low | window inventory / error suggestion | mine, later |

## Loop
- [x] Build v1.3.0 baseline binary (bench-v130, 930cb49a, 186 s)
- [x] Probe launch latency vs window appearance (scratchpad probe/launch_v130.txt, launch_pr5.txt)
- Found: T3 Code holds a full-screen Space. A plain `launch` then opens the app off-screen: `launch` waits out its 1.5 s grace and returns no window, and the next snapshot fails WINDOW_NOT_FOUND (4/4, Calculator and Weather). `launch --activate` works. The 1.3.0 runs had the desktop Space current. Bench prelude `desktop_space.sh` (sh: step, outside measured time) restores that before each entry.
- Found: launch returns when the CG window shows; the AX window exists 0.05-1.0 s earlier (Weather ~0.9 s).
- Found: a native app's first snapshot = cold walk (Weather 0.76 s vs 0.27 warm) + activation + second walk (0.28 s).
- [x] #5, #7, #9 merged on main (cce8fa1c). Post-#5 CLI medians in the paired runs: Weather 3.7 s (1.3.0: 6.0), Settings 4.5 (6.7).
- [x] Launch poll cap (perf/launch-window-poll 24d81dcd, local only): alternating cold launches, n=7, main 479df516 vs fix. Medians flat (Weather 1.25/1.26, Settings 0.89/0.87, Calculator 0.66/0.67, Font Book 0.98/0.94 s); only maxima drop (Settings 1.12 -> 0.94, Font Book 1.54 -> 1.01). The 0.4 s overshoot came from the coarse shell probe. Parked: no PR.
- [x] Skill named-path (perf/skill-named-path): v1 partial A/B vs main 479df516, n=3 (stopped): settings median 4 turns both arms; tokens 132.7K -> 123.8K suite; models step one click per call and cut the -w read with head -N. v2 wording (12bfcdbc) built; A/B queued after #48.
- [x] Control: #48 (48f7cfa8, contains main cce8fa1c) paired A/B vs main cce8fa1c, n=8 per task per side, started 15:40 PST (plan-p48.txt). Merge only if not worse on tokens and turns; else split the synopsis fixes and `every_flag_of_a_synopsis_command_appears_in_its_form` into a smaller PR.

## Results (2026-11-09, wrap-up 17:25 PST)

- #48 A/B (main cce8fa1c vs #48 48f7cfa8, n=8/task/side, 24/24 both): sums 47.5 -> 51.3 s, 126.1K -> 144.0K tokens (+14%), turns 9.5 -> 10. Settings 50.8K/4 -> 73.1K/5: the core-skill synopsis's `find --first` example led runs to the single `data.match` shape and an extra snapshot. NOT merged; #48 left open for its owner.
- #82 MERGED (b011749e): #48's synopsis + flag fixes moved to references/commands-interaction.md (on demand), plus its 3 synopsis tests incl. `every_flag_of_a_synopsis_command_appears_in_its_form` (invert-verified). Main CI on b011749e: Test (Windows) failed once in `context::scope_tests::batch_item_with_failed_parent_trace_writes_to_its_own_session_segment` (core trace test, unrelated); rerun green.
- #85 MERGED (644dc78e): skill "take a named path and the answer in one call". Paired vs main cce8fa1c, n=8: Settings 4 -> 2 turns, 51.1K -> 25.8K, 19.2 -> 12.4 s; sums 48.6 -> 43.2 s, 119.0K -> 95.4K, 9 -> 7 turns, 24/24 both.
- 1.4 protocol run, v1.3.0 (930cb49a) vs main+#85 (2025f95d tree), n=8/task/side, one session, 24/24 both: sums of medians 60.4 -> 42.8 s, 119.1K -> 96.1K tokens, 9 -> 7 turns; means 59.9 -> 45.9 s. Target (beat 43.2 s and 118K, 24/24) met in this pairing. v1.3.0 itself ran 17 s slower than its release run (model latency drift).
- Parked, pushed, no PR: perf/launch-window-poll 24d81dcd (no median gain, n=7); perf/skill-trim-os-detail 0296fb24 (only -452 B, ~-110 tokens/call; below A/B noise).
- Found: a merge commit on perf/skill-named-path ran the full pre-commit hook outside serial_cargo (SKIP_PRECOMMIT left off); it passed and no other cargo was running. Use `SKIP_PRECOMMIT=1` on merges and run the hook via serial_cargo.

## State at stop
- Nothing running. Desktop lock free. Bench apps quit. No partial A/B data: every result above is complete (the stopped v1 run, snp1-3 of plan-snp.txt, is labelled partial and used only as a pointer).
- Result rows: ~/.t3/worktrees/agent-desktop/perf-speed-cost/bench/runs/results.jsonl, run ids >= 1109151200 (snp), 1109154030 (p48), snv2 and r14 after it.

## Next (ranked)
1. Native first observation: skip the second full walk after AXEnhancedUserInterface for non-Chromium apps without a web area (~-0.8 s/suite). Needs a check across native apps, a non-Chromium web engine, and an Electron app before any change.
2. WINDOW_NOT_FOUND after a plain launch while a full-screen app holds the screen: suggest `launch --activate`.
3. Weather/Calculator are at their turn floor; remaining time is model latency and app cold start.

## Resume
Rebuild the trees (scratchpad and bench worktrees may be gone after the restart), then rerun the protocol against current main:
```bash
cd ~/.t3/worktrees/agent-desktop/perf-speed-cost
git -C ~/Projects/agent-desktop fetch origin && git -C ~/Projects/agent-desktop worktree add --detach ~/.t3/worktrees/agent-desktop/bench-main-next origin/main
bash -c 'source bench/cargo_env.sh; bench_build ~/.t3/worktrees/agent-desktop/bench-v130; bench_build ~/.t3/worktrees/agent-desktop/bench-main-next'
# plan: 8 rounds copied from bench/plan-final-ab.txt with wt-upstream -> bench-v130 and wt-final -> bench-main-next,
# each entry preceded by sh:<desktop_space.sh> (launch Calculator --activate, then macctl quit Calculator) while T3 Code is full-screen
bash bench/orchestrate.sh <plan file>
```
The plan generator (mkplan.py), the desktop prelude (desktop_space.sh), the probes and ab_summary.py are copied under bench-tools/ on this branch.
