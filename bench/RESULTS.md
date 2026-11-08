# Speed and token cost on the 5-task Mac benchmark

Interim report, 2026-11-07. The fix A/B rounds are still running; sections marked *pending* fill in from them.

## Conclusion so far

- Three of the five tasks are already fast: Weather, System Settings and Calculator take 14–23 s each with 27K–123K input tokens and pass every run. Two tasks hold almost all the cost:
  - **Finder folder:** about 280 s, 1.1M input tokens, 43 model calls, 2 of 14 runs pass.
  - **TextEdit Save:** about 230 s, 1.3M input tokens, 45 calls, 6 of 10 runs pass.
- In both slow tasks the tool waits out a fixed 5 s budget, fails or reports something the model can't trust, and the model then retries around it. The model's own thinking is a symptom of this, not the cause.
- Each slow task has a mechanical cause, and each has a fix branch on the fork (see [Ranked fixes](#ranked-fixes)).
- The skill-guidance variants (configs b–e) move the suite by at most 15% in time and 20% in tokens. None of them fixes either slow task, so prompt guidance is not the lever.
- About 41% of input tokens is the fixed prefix (system prompt, tools and skill), re-sent on every call; another 16–27% is snapshot output. Cutting the number of calls in the two slow tasks removes the most tokens.

## Targets

Torben's 5-task benchmark:

| Tool | Tasks passed | Time | Tokens |
|---|---|---|---|
| Aside Computer Use | 5/5 | 106 s | ~1M |
| agent-desktop fork, before these fixes (Torben's run) | 5/5 | 745 s | 5.6M |
| agent-desktop fork, this bench, config a, before fixes | 11/14 runs | 573 s | 2.59M input + 21K output |

These are vendor claims, not measured here: Aside says it is 2.7x faster and uses 65% fewer tokens than agent-desktop.

This bench's time is the sum of the per-task median wall times. Torben's runs and this bench differ in model, harness and run count, so compare ratios within one table, not across tables.

## Method

- **Driver:** `bench/driver.py` runs each task through `claude -p` with Bash only, and a wrapper logs every agent-desktop call with its time and output size.
  - The model is Opus 5.5; a Sonnet 5.5 round on config a is *pending*.
  - Each run starts from a reset desktop and is checked by independent observation: the file system for Finder and TextEdit, a fresh snapshot for Calculator.
- **Configs:**
  - a: the shipped skill.
  - b: lean guidance.
  - c: batch guidance.
  - d: no-resnapshot guidance.
  - e: b, c and d combined.
  - m, mx, x: MCP, MCP without the skill, and CLI without the skill (*pending*).
- **Time:** wall time = CLI time (from the wrapper log) + model time (`duration_api_ms`) + other (harness and start-up).
- **Tokens:** input tokens are split by exact per-call deltas. JSON tokenizes at 2.07 bytes per token, calibrated against real `claude -p` usage (`bench/token_calibrate.py`).
- **Desktop lock:** GUI work takes turns with the browser session through `~/.agent-desktop/bench.lock` (`bench/orchestrate.sh`).
- **Re-scored runs:** four TextEdit runs are re-scored as failures in `bench/runs/corrections.jsonl`. Each saved a second copy named `:Users:…:note-….rtf` in iCloud, and the old check looked only at the target folder.

## Where the time and tokens go (config a, median of runs)

| Task | Runs passed | Wall s | Model s | CLI s | Calls | Input tokens |
|---|---|---|---|---|---|---|
| Weather | 3/3 | 15.2 | 8.0 | 6.6 | 4 | 56,503 |
| System Settings | 3/3 | 22.4 | 15.3 | 6.1 | 6 | 75,541 |
| Finder folder | 1/3 | 286.1 | 160.3 | 122.3 | 43 | 1,122,600 |
| TextEdit Save | 1/2 | 234.4 | 106.0 | 124.6 | 45 | 1,296,403 |
| Calculator | 3/3 | 14.8 | 8.2 | 4.8 | 3 | 38,365 |

Input tokens by source, config a, all runs:

| Source | Share |
|---|---|
| System prompt and tools | 31% |
| Skill | 10% |
| Snapshot output | 27% |
| Other command output | 13% |
| Skill references | 3% |
| Model output re-read as history | 14% |
| Task prompt | 1% |

CLI calls of 4.5 s or more, all configs (10–14 runs per task):

| Task | Command | Calls | Total s | Cause |
|---|---|---|---|---|
| Finder | click | 90 | 454 | post-action surface walk runs to its 5 s deadline |
| Finder | find | 30 | 264 | 60-folder window over the 3 s observation budget |
| TextEdit | find | 30 | 211 | find in the Save sheet times out at 5 s |
| TextEdit | snapshot | 21 | 198 | Save sheet snapshots take 3–11 s |
| TextEdit | click | 23 | 115 | pop-up actionability waits 5 s, then `TIMEOUT` |
| Finder | focus-window, list-surfaces, right-click, select | 52 | 291 | same 5 s surface walk |

Full tables: `python3 bench/analyze.py`.

## Ranked fixes

| Rank | Problem | Branch | Evidence | Expected win | Status | Prior art | Upstream |
|---|---|---|---|---|---|---|---|
| 1 | Every Finder click, select, expand and press costs about 5 s. The post-action surface check got the whole action budget, and the context-menu walk visits every app node, times out, and the result is thrown away. | `perf/settled-surface-budget` | 90 Finder clicks at 5 s; unit tests invert-verified | up to ~35 s CLI per Finder run, plus the retries it causes | fork PR #4; live A/B: Finder row click median 2,424 → 1,092 ms, 7/7 verified | upstream #399 fixes a different Finder `list-surfaces` failure (vanished nodes; the fork already reacquires); I#416 (re-scan cost) is related | `needs-issue-first`: the post-action report gains an `app/menubar` entry marked `unclassified`, and its budget changes |
| 2 | The new folder stays "untitled folder": `set-value` on Finder's rename field returns ok and reads back the new name, but Finder never commits it. | *pending probe* (`bench/probe_rename.sh`) | 8 of 10 Finder failures | most Finder failures and retries | probe queued | none found | not built |
| 3 | In the TextEdit Save sheet, a click, select or expand on a pop-up waits out 5 s, then fails with `TIMEOUT`. The pop-up omits `AXWindow`, so its offscreen state stays unknown. | `fix/popup-window-viewport` (carries upstream #453) | ~170 s of 5 s timeouts over 10 runs | ~17 s CLI per TextEdit run, plus retries | fork PR #6; live A/B: 5,095 ms `TIMEOUT` → 183 ms `delivered_verified` | upstream #453 (open) | `overlaps #453` (carry, plus review fixes) |
| 4 | A full path in the Save As field saves a `:`-named copy into iCloud, and `set-value` still reports success. | fork PR #3 (docs, merged) | 4 of 10 TextEdit runs; browser x1 | correctness; fewer retries | merged | upstream I#360 is related | `ready` (merged in the fork as #3) |
| 4b | Finder rename: `set-value` on the new folder's name field, then a headed Return, reports `delivered_verified`, but the folder on disk stays "untitled folder". The wrong outcome is reported as success. | *pending probe v3* (`bench/probe_rename.sh`: set-value and type with a headless Return) | v2, 2026-11-07: set-value plus headed Return is `delivered_verified` with `application_commit: not_verified` and no rename; headed paced `type` gets `TIMEOUT` (field never focused); per-key headless presses plus Return rename but drop letters (`renamd-pres-key`) | the same as rank 2 | probe v3 queued | none found | not built |
| 5 | Snapshot JSON is 16–27% of input tokens at 2 bytes per token. | `perf/outline` | outline is about one third of the JSON tokens on the fixture apps | 10–18% of input tokens | matrix A/B *pending* | none found | `needs-issue-first`: changes the snapshot output contract |
| 6 | A cold native app pays a fixed 2 s settle meant for Chromium renderers. | `perf/cold-settle-chromium` | V8-snapshot detection marks the 15 installed Chromium apps and no native app | 2 s per first snapshot of a cold native app | fork PR #5; cold A/B: Weather 3.2 → 1.1 s, Calculator 2.3 → 0.35 s | carries antoniel's `286ed072` (described in upstream issue `#416`); upstream `#254` (closed) is related | `needs-issue-first`: changes default timing, and `launch` now reports `renderer: "chromium"` for Chrome-family apps; carries antoniel's `286ed072` (upstream issue `#416`) |
| 7 | Clicking a Finder folder item opens it but reports `ACTION_FAILED`. | `fix/open-closed-sheet` (carries upstream #541, plus a window-title signal tied to the target's label) | -25205 after navigation | fewer false failures and retries | fork PR #9; unit tests invert-verified; live check, perf compare and E2E queued | upstream #541 (open) | `overlaps #541` (carry); the window-title signal stacks on it |
| 8 | A pending macOS consent prompt hangs any command, 175 s observed. | `fix/automation-probe-deadline` | unit tests; the hang did not reproduce while the prompt was already shown | reliability | review done | upstream #388 bounds the prompting path, not this check | `needs-issue-first`: `automation` reads `unknown` outside `status`, `permissions` and `batch`; complementary to `#388` |

Upstream status. Nothing goes to upstream yet. Each fix stays upstream-ready: its own branch from fork `main`, no dependency on fork-only browser work, and upstream references written as inline code only. The status labels:
- `ready`: can go upstream as is.
- `needs-issue-first`: changes the JSON contract or a default behavior, so an upstream issue should agree on it first.
- `overlaps #N`: carries or builds on an open upstream PR.
- `fork-only`: stays in the fork.

The bench itself (`perf/speed-cost`) is `fork-only`.

## Background and cold start

*Pending:* the extras round (`bench/run_extras.sh`) measures a task with another app in front, the focus changes it causes (`bench/background.py`, read through `bench/focus_probe.swift`), and cold-start cost per app (`bench/cold_start.sh`).

## Reproduce

```bash
cargo build --release -p agent-desktop
python3 bench/cli_latency.py
python3 bench/driver.py --task finder --config a --model claude-opus-5-5
bash bench/orchestrate.sh bench/plan-phase1d.txt
python3 bench/analyze.py
```

Run outputs (`bench/runs/`, `bench/out/`) are gitignored: they hold snapshots and model transcripts of the desktop.
