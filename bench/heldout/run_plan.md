# Held-out run plan, Wednesday 2026-11-11

Report due before Thursday 00:00. Every model run uses `claude -p` on the subscription (Claude Code 2.1.321 on the Mac), the same driver as the existing bench. No API key, no money.

## Before the first run (Mac, speed session)

1. George has quit TextEdit and Preview, and Google Chrome is closed. The runners stop on their own before the model starts if any of these shows a non-bench window, and they never touch it.
2. Run `bash heldout/preflight.sh`. It must print `ok` on every line. Read the two permission-status blocks:
   - If Peekaboo reports Accessibility missing, ask George (ARMS.md); otherwise drop `peekaboo` from `NATIVE_ARMS`.
   - Cua stays out on the Mac unless George chose option 1 in ARMS.md: `NATIVE_ARMS="ad-14 ad-up peekaboo"`.
3. The task file is already frozen (sha256 in `TASKS_FROZEN.md`). `run_all.sh` refuses to start if it changed.

## Order

- **Pass 1, Opus 5.5 (`claude-opus-5-5`), n=5:** rounds 0–4. Each round runs every task once per arm, and both the arm order and the task order rotate each round. Native arms come first in a round, one desktop-lock hold per arm. Then the web arms: each one is a single `run.py` call, which takes the lock itself, so other sessions can interleave between arms.
- **Pass 2, Sonnet 5.5 (`claude-sonnet-5-5`), stretch:** the same rounds, only after pass 1 is complete.
- Run it detached, so a session restart cannot kill it:

```bash
cd ~/.t3/worktrees/agent-desktop/perf-speed-cost/bench
NATIVE_ARMS="ad-14 ad-up peekaboo" WEB_ARMS="ad-14 ad-up bh pw" \
  perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV' bash heldout/run_all.sh claude-opus-5-5 0 4 all \
  > heldout/out/opus.log 2>&1 &
# then, if there is time:
perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV' bash heldout/run_all.sh claude-sonnet-5-5 0 4 all > heldout/out/sonnet.log 2>&1 &
python3.11 -I heldout/report.py --model claude-opus-5-5 > heldout/REPORT-opus.md
```

## Estimated wall time (Opus, desktop time only, lock contention not included)

Per-run estimates come from the closest measured runs: the 1110-evening web runs (ad-web, AX, browser-harness), the fork-main native rows in `RESULTS.md` (seeded TextEdit 52–75 s, Settings reads 18–29 s), plus about 8 s of harness setup per native run and about 12 s of Chrome start and stop per web run. Peekaboo, Cua and Playwright CLI have never run here, so their numbers are guesses.

| Arm | Native s/run | Native ×8 tasks ×5 | Web s/run | Web ×10 tasks ×5 |
|---|---|---|---|---|
| ad-14 | 50 | 33 min | 35 | 29 min |
| ad-up | 70 | 47 min | 100 (AX-only web is slow) | 83 min |
| peekaboo | 90 (guess) | 60 min | — | — |
| cua (AWS only) | 90 (guess) | 60 min | — | — |
| bh | — | — | 45 | 38 min |
| pw | — | — | 45 (guess) | 38 min |

- Pass 1 on the Mac without cua: native ≈ 2.3 h, web ≈ 3.1 h, **≈ 5.5 h in total**. With cua it is ≈ 6.5 h.
- Pass 2 (Sonnet) adds about the same again. Sonnet is usually a little faster, so ≈ 5 h.
- The worst case is bounded by the caps: 300 s per native run, 600 s per web run.
- To finish by 23:00 with a report, pass 1 must start by **09:00**. Pass 2 then runs until the cut-off, and the report includes whatever rounds finished.

## If the usage limit hits

Drop in this order, and say so in the report:
1. **The Sonnet pass.** Stop it and report Opus only.
2. **Playwright CLI (`pw`)**: `WEB_ARMS="ad-14 ad-up bh"`.
3. **Peekaboo**: `NATIVE_ARMS="ad-14 ad-up"`.
4. If it still hits: n drops from 5 to 3 (rounds 0–2) for the remaining arms. ad-14 and ad-up always keep the same n.

Resume after a limit reset with the next round: `run_all.sh <model> <next_round> 4`. Rows are appended, and the report reads whatever rows exist.

## After each round

- `tail heldout/out/opus.log`. A run that fails in setup leaves no row: rerun that arm and round by hand, `--round N`.
- Window hygiene: `python3 bench/post_probe_check.py heldout` must be clean. Bench documents are `seed-ho-*` files in `~/agent-desktop-bench/bench-ho-*`.
- Disk space: driver rows are marked invalid when the run sees ENOSPC or less than 2 GB free.

## Report

`python3.11 -I heldout/report.py --model claude-opus-5-5 --host mac`, then again with `--model claude-sonnet-5-5`, and again with `--host aws` for AWS rows. The report puts CLI/tool time first, then wall time, tokens, turns and pass rate. It gives min/median/max per task and lists every task and metric where 1.4 loses.

## Bench a candidate binary (remote bench Mac, one command)

From the control Mac, with a release build of the candidate (`serial_cargo.sh cargo build --release -p agent-desktop -p agent-desktop-macos --bins` leaves `agent-desktop` and `agent-desktop-macos-helper` side by side in the checkout's target dir, `serial_cargo.sh --target-dir`):

```bash
bash ~/.t3/worktrees/agent-desktop/perf-speed-cost/bench/heldout/bench_candidate.sh <label> <path-to-agent-desktop> [task ids|all] [n]
# e.g. NOTE="PR #140 abc1234" WAIT=1 bash .../bench_candidate.sh ad-pr140 <target>/release/agent-desktop hp3,hw3 5
bash .../bench_candidate.sh --status <label>     # tail the remote log
bash .../bench_candidate.sh --collect <label>    # fetch rows, write results/candidates/REPORT-<label>.md against every arm
bash .../bench_candidate.sh --stop               # stop the running pass after its current run
```

- `<label>` becomes the arm name in every row (lowercase, digits, dots, dashes; not a built-in arm). One label holds one binary: the script refuses a label whose remote copy has another sha256. Use a fresh label per build.
- Tasks: comma-separated ids (`hn1`..`hn8`, `hw1`..`hw7`, `hp1`..`hp3`) or `all`; `n` rounds (default 5). Every round runs each task once in the Sonnet pass's rotation, with the same seeds (web: `web_seed(task, round)`), prompts, caps and Chrome flags. The candidate is prompted exactly like `ad-14`.
- Spend: the script prints an estimate (native $0.10, web $0.03 per run, Sonnet 5.5 list price) and refuses above `CAP_USD` (default 15). On the remote, `remote/cand_pass.sh` sums the pass's real list cost after every run and stops past `CAP_USD`.
- It refuses while another pass runs. It syncs `bench/heldout` to the remote mirror (not `runs/`, `out/` or the Swift helpers, whose compiled copies are already there), copies the binary and its helper into `heldout-tools/candidates/<label>/` with a VERSION line, and starts the pass in the GUI session through benchrun.
- Results land in `~/.agent-desktop-bench-target/remote/results/candidates/`: `<label>-results-native.jsonl`, `<label>-results-web.jsonl` and `REPORT-<label>.md` (focus arm = the candidate, compared with every Sonnet arm).
- Time: about 11 minutes per round of all 18 tasks, so `all 5` is about an hour.
