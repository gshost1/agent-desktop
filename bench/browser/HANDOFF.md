# Browser lane handoff (2026-11-08)

The fork's agent-desktop now drives Chrome pages through its own snapshot, ref and action loop over CDP. Web surface #28, downloads #47 and the no-sleep skill line #60 are merged on the fork (main 95fea935). This file holds the bench numbers, how to rerun them, what is open and the traps hit along the way. The raw transcripts stayed in the old session's scratchpad and are not in the repo.

## Medians

Sonnet driver, Bash only, headed Chrome on a throwaway profile, 3 runs per task unless noted. Input tokens count input plus cache reads and writes.

| Task | AX only (n=1) | agent-browser cdp | hybrid (n=1) | ad-web, #47 build | ad-web, #60 skill |
|---|---|---|---|---|---|
| w1 read | 16.6 s, 70K, 6 turns | 6.6 s, 19K, 3 | 7.0 s, 13K, 2 | 5.9 s, 22.3K, 3 | — |
| w2 form | 88 s, 607K, 27 | 10.4 s, 38K, 5 | 10.3 s, 37K, 5 | 11.8 s, 45.7K, 5 | 11.2 s, 44.1K, 5 |
| w3 wizard | 41 s, 216K, 13 | 22 s, 89K, 11 | 17 s, 78K, 10 | 16.7 s, 52.6K, 6 | 14.9 s, 52.9K, 6 |
| w4 300-row table | 20 s, 59K, 5 | 7.7 s, 25K, 4 | 6.9 s, 26K, 4 | 8.7 s, 40.1K, 5 | — |
| x2 page to Calculator | 91 s, 315K, 14 | — | 18.5 s, 67K, 7 | 17.4 s, 67.7K, 7 | — |

- Web sum w1–w4: AX 166 s / 952K, agent-browser 47 s / 171K, hybrid 41 s / 154K, ad-web 43.1 s / 160.8K. The unified loop is −74% time and −83% tokens against AX, at parity with agent-browser.
- ad-web passed 15 of 15 runs on the #47 build and 6 of 6 on the #60 build. The #60 skill line took agent sleeps in w2 and w3 from 13 to 0.
- x1 (save to TextEdit) and x3 (download, then rename in Finder) are held by the personal-folders rule. x3 is the task #47 targets.

## How to rerun

All on the Mac. Chrome must not be running, and the desktop lock must be free.

```bash
cd ~/.t3/worktrees/agent-desktop/feat-unified-browser
SC=~/.t3/worktrees/agent-desktop/perf-speed-cost/bench/serial_cargo.sh
TD=$HOME/.agent-desktop-bench-target/<worktree-name>
bash $SC env CARGO_TARGET_DIR=$TD cargo build --locked --release -p agent-desktop
python3.11 -I bench/browser/server.py 8791 &
bash bench/browser/rounds.sh 0 2 -- --channels ad-web --tasks w1,w2,w3,w4,x2 --bin-dir $TD/release --out <bench-owned dir>
python3.11 -I bench/browser/summarize.py <bench-owned dir>/runs.jsonl
```

- `rounds.sh` takes the desktop lock once per round through `lock.py`, so other sessions can interleave.
- `run.py` starts the ad-web channel's Chrome with `agent-desktop launch --cdp --download-dir`, under each run's own `AGENT_DESKTOP_HOME`. Each run gets a fresh profile, and Chrome is stopped before that profile is deleted.
- Run a long bench detached (`perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV' bash script.sh &`). A Claude Code session restart kills ordinary background jobs; that killed three builds on this night.
- `python3 -I -m unittest discover -s bench/browser -p 'test_*.py'` covers the harness itself (15 tests).

## Open items

1. **macOS E2E on a fresh main build**, once fork PRs #12 (Python version check, `lib.sh` empty-array fix) and #32 (window titles from AX) are both on main. With Python 3.11 but without #32, the suite stops at `tests/e2e/safe-semantic.sh:345` ("fixture did not expose one unambiguous owned window").
2. **Real-profile CDP daemon: on hold until George says go.** It is the only way into his real Chrome. Its Allow prompt appears once per browser-level connection, so per-call CLI connections would prompt on every command.
3. **Not built, below the 5% stop rule:**
   - snapshot to a file (B): snapshot text is 0.6–1.9K of 22–68K input per run;
   - batch stop conditions (C): agents already chain the action and the snapshot in one call;
   - dropping a redundant text child (D): the web channel already drops it (`crates/web/src/tree/frame_build.rs`), and on AX it would save 0.3–0.8% of snapshot bytes.
4. **A post-click settle wait was measured and dropped.** Without it, the snapshot after a click already waits for the navigation: 620–640 ms on a 600 ms page. The settle only moves that wait into the click and adds about 100 ms to every click that starts nothing.
5. **Two of the 15 ad-web runs lost a turn** when Claude Code's Bash sandbox refused a `batch` whose JSON spliced in a shell variable ("Brace expansion"). Not built; a skill line could say to write refs literally.
6. **Leftovers George decides on:**
   - T3 worktrees `t3-c80c0c8a` and `t3-fa04024a`, both on merged branches;
   - bench target dirs under `~/.agent-desktop-bench-target`;
   - a stray python3.9 `bench/browser/server.py 8791` from an earlier session.

## Traps

- **Personal folders.** No bench, probe or check may open, list, snapshot or screenshot ~/Pictures, Documents, Desktop, Downloads, Movies, Music, iCloud, Photos, Mail or Messages. Downloads go to a folder the bench creates.
- **Bench Chrome.** Never use George's real profile. Use a fresh empty profile per run, created before launch and deleted only after every Chrome process on it has exited. Pass `--no-first-run --no-default-browser-check`. A dialog on screen fails the run.
- **Every cargo command goes through `serial_cargo.sh` with a per-worktree `CARGO_TARGET_DIR`.**
  - A shared target dir reused other worktrees' builds of path crates; one gate ran main's core.
  - In zsh, wrap it in `bash -c`, because zsh does not split `$TD`.
  - `serial_cargo` waits while the desktop lock is held, so build before taking the lock.
- **Run clippy before every push, and count struct fields (≤ 7) and function parameters (≤ 5).** #47 lost a CI cycle to an unused import, and left `LaunchArgs` at 8 fields (speed's #54 regroups it).
- **Never AppleScript System Events.** The E2E needs `bench/guard` first on `PATH`, plus `python3` resolving to 3.10 or newer (`/opt/homebrew/bin` holds only `python3.11`, so link it as `python3` in a folder first on `PATH`). Skip the notifications and os_surfaces scenarios, which open personal apps.
- **Merge rule (fork only):**
  - the head contains main;
  - CI is 13 of 13 green and GitHub reports CLEAN;
  - if main moved since, no overlapping files (Cargo files, `.github/**`, core `lib.rs`, AGENTS.md and touched skills always count);
  - `release.yml` is `disabled_manually`;
  - then `gh pr merge N --squash --delete-branch`, and watch main's CI.
- **`VirtualClock::freeze()`** advances only through `deadline::sleep`. A test stub that never sleeps hangs the wait loop.

## Night log (from the session's TASKS.md)

## Unattended night 2026-11-08 (from ~08:15Z; George back ~16:00Z)
Rules: red → fix and rerun; same failure twice → write it here and move on; push at checkpoints; fork merge only under the merge rule; serial_cargo; desktop lock for anything on screen; no subagents while load > 8; no TCC/System Settings.
Priorities: (1) double-click/scroll, press Return, address bar; (2) check 5; (3) web-surface PR merged; (4) A-D measured.
- [x] double-click/scroll cdp_no_pointer fixed (3518d762, check 5 v2 pass).
- [x] press Return + address bar (6e975d59; check 5 v3: page q=hello, omnibox /landed?omnibox): root cause = OS key goes to Chrome's keyboard focus (address bar), not DOM focus set over CDP; AX set-value on the omnibox doesn't move keyboard focus either. Fix: `press COMBO --ref REF` (core press → ref-action pipeline, Action::PressKey; web ref = Input.dispatchKeyEvent through the page; native ref = platform focus-then-key). Restored the web press path (FFI execute_by_ref uses it; my "dead path" removal was wrong). Tests: press_ref_tests.rs. Docs: keyboard.mdx, commands-interaction.md, web skill. Live: check 5 P/O now use --ref.
- [x] review fixes 2c884d50 + press --ref 6e975d59 + page choice (delegate 37edff60, merged 50020ea3) + matches_bounds cleanup 269039e9; every new test invert-verified in the per-worktree target.
- Shared-target taint (found 08:30Z; control: re-run, mark): results built through serial_cargo's shared target since ~04:50Z are SUSPECT (possibly a wrong core):
  - check 5 v2 binary (feat/web-downloads e66b9aaa, release, 07:34Z) → all check 5 v2 numbers suspect; superseded by v3 from a per-worktree build.
  - 3518d762 protocol_delivery invert-verify, bd9c69ff live_labels run + invert-verify, the 3518d762 gate → re-run in t3-c80c0c8a's own target.
  - delegate fix/web-page-choice: told to re-run in its own target.
  - 08:20Z gate (bylatkvv2): confirmed wrong core (main's), discarded.
- [x] check 5 v3 09:14Z, binary feat/web-downloads 29ff561a (per-worktree target): all skill examples pass; press --ref page+omnibox pass; 4/4 downloads in bench folder; no-download click 101-132 ms (was 321-442); Chrome front at launch only (self-activates; skill line e30a1eb0); lock held 30 s. (P/O via --ref, front after launch/open-url, D dropped: Launch Services has no --cwd; connection scope covered by download_profile unit tests).
- Found: main's `dispatch::scope_end::tests::a_strict_trace_failure_at_the_batch_end_keeps_every_entry_result` (#21) fails under a parallel full run with `lock_timeout` (2 of 5 runs), passes 8/8 alone. Load-sensitive lock wait; main's lane → control.
- Found: `launch --cdp` validation changed with the shared core helper: needs HTTP 200, ws only (no wss), accepts localhost/[::1]. No E2E covers launch --cdp.

- Harness: stray-save rule (control thread) — a run fails if it leaves fresh files in iCloud TextEdit Documents or any fresh name starting with ':' (watched: iCloud TextEdit, ~/Documents, ~/Desktop, ~/Downloads, run work dir); strays are moved to results/strays/<run>/, never deleted. Speed session owns the save-path fix; x1 will use its probe_savepath.sh result.

- [x] /security-review (agent): 1 finding ≥7 fixed e08b278a: checked port ≠ connected port (registry URL port, DevToolsActivePort re-read) → cdp_endpoint_mismatch; 2 tests invert-verified. Lower: open-url schemes (policy), UA heuristic, 256 MiB msg cap — in PR risks.
- [x] perf A/B fixture 5 rounds head e08b278a vs 89a3723e: neutral (all head p50 inside base samples).
- Found (same failure twice → recorded): macOS E2E on this Mac stops at "fixture did not expose one unambiguous owned window" — same on fork main and every speed branch (bench/out/e2e-*.log). Needs python ≥3.10 (use ~/.local/bin/python3.11 + bench/guard on PATH); PR #12 carries prerequisites. Local run skipped notifications + os_surfaces (personal apps rule).
- [x] PR #28 squash-merged 55c69093 (CI 13/13 on e6fa650e, CLEAN; main moved only by #43, no overlap). #44 (flaky-test freeze) closed: main's #45 made the same change.
- [x] feat/web-downloads rebuilt on main without force-push: tree = main + downloads diff, gated by the full pre-commit hook (core 1803, web 251 lib tests). NOTE: that hook ran cargo directly, not via serial_cargo. Merge commit 2e2623c2 (parents 29ff561a + 55c69093).
- [x] PR #47 web-downloads squash-merged 0a5ff966 (CI 13/13 on a8f8c0a8, CLEAN; main then moved only by #49, wait_scheduler.rs + wait_event_tests.rs, no overlap; release.yml disabled_manually). Fixes on the way: security 43f93c71; Windows compile 9a3455b3; Windows /tmp-path test adb81d3f; unused import 453ff299 (pushed without local clippy while the lock was held: cost one CI cycle). Worktree + local branch removed.
- [x] C (settle wait) measured, code NOT kept: settling every web click/double-click/press (100 ms start window, then until load, cap 3 s) made a click onto a 600 ms page return at ~750-800 ms, but WITHOUT it the click returns in 25-60 ms and the snapshot right after takes ~620-640 ms and already shows the new page (link and script navigation, both scopes). Net: settle only moves the wait and adds ~100 ms to every non-navigating profile-scope click (4.7 ms -> 106 ms). Kept instead: skill line "snapshot right after the click, no sleep" + live test live_navigation_settle.rs pinning it (branch feat/web-snapshot-after-click 9ef29ca9). Batch stop conditions: not built; bench agents already chain action && snapshot in one Bash call (1 turn per step), and batch was used only for fills.
- Found: 2/15 ad-web runs lost a turn to Claude Code's Bash sandbox refusing a batch whose JSON splices a shell variable ("Brace expansion"). <5% of the matrix; recorded, not built.
- [x] B (snapshot to file + summary): NOT built. ad-web snapshot text is 0.6-1.9K tokens per run of 22-68K input (<5%); on w4 (300 rows) Claude Code already spilled the big output to a file and the agent grepped it.
- Found (control, 14:05Z): #47 left CLI `LaunchArgs` at 8 fields (limit 7). Speed's #54 regroups the CDP + download args; don't touch LaunchArgs in parallel. No script enforces the field/param limits: count by hand before push.
- [x] main CI green on the #47 merge 0a5ff966.
- [x] No-sleep skill bench (9ef29ca9, 14:16-14:18Z, run detached after 3 session restarts killed the build): w2/w3 6/6 pass, sleeps 13 -> 0; w3 16.7 -> 14.9 s, w2 11.8 -> 11.2 s, tokens flat. PR #60 opened (head ee3bb0e3, main merged in).
- [x] D (redundant text child): NOT built. Web already drops it (crates/web/src/tree/frame_build.rs:73). AX bench snapshots: 0.3% (ax) / 0.8% (hybrid) of snapshot bytes → <0.3% of input; below the 5% stop rule. `-i` already prunes non-ref text leaves.
- [x] A (skill weight): the ad-web channel reads only `skills get web` (5.9K chars) vs desktop+platform (15.0K) → −61% skill text. Effect to be measured by the ad-web bench below.
- [x] bench: ad-web channel now launches Chrome via `agent-desktop launch --cdp --download-dir` under the run's AGENT_DESKTOP_HOME (255588da); before, it had no endpoint.
- Found (same failure 3x → recorded, speed lane): E2E on #47 (12:40Z, release build of 0adf02e2, Python 3.11.16 first on PATH) still stops at safe-semantic.sh:345 "fixture did not expose one unambiguous owned window": capture_fixture_identity empty for 30 tries x 0.2 s. So python 3.9 is not the only cause on this Mac. Also harness bug: lib.sh:288 `cleanup_files[@]: unbound variable` on exit (empty array under set -u, bash 3.2). Log: scratchpad/e2e-dl.log.
- [x] ad-web bench 12:41-12:46Z (release build of #47 at 0adf02e2, Sonnet, headed, lock per round): 15/15 pass. Medians: w1 5.9s/22.3K/3t, w2 11.8s/45.7K/5t, w3 16.7s/52.6K/6t, w4 8.7s/40.1K/5t, x2 17.4s/67.7K/7t. Web sum w1-w4: 43.1s / 160.8K vs AX 166s/952K (-74% time, -83% tokens), agent-browser 47s/171K, hybrid 41s/154K. x2 vs AX 91s/315K: -81% / -79%. Results: scratchpad/results-adweb/runs.jsonl.
- Control correction (12:20Z): the E2E fixture failure was python3 3.9 crashing on `str | None`; fork PR #12 adds a version check. Note: /opt/homebrew/bin has only python3.11, no python3, so my e2e-python symlink dir is what puts 3.11 first.
- [ ] Leftover worktrees: t3-c80c0c8a (feat/web-surface, merged) and t3-fa04024a (fix/web-page-choice, merged) are T3-managed; left for George. Bench target dirs under ~/.agent-desktop-bench-target stay (rm denied).
