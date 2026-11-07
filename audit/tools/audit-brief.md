# Shared brief for state-machine audit agents (READ-ONLY)

Repo: /home/george/Projects/agent-desktop (main = b43ac4f6). Rust CLI + C lib reading native accessibility trees.
Read first: AGENTS.md, CONCEPTS.md, docs/json-output.md, docs/solutions/ (skim titles, read relevant ones).
Prior art: /tmp/claude-1000/-home-george--t3-scratch-2026-11-07-goal-find-every-implicit-state-1f4e3925/11164cc5-bf9f-4a7c-bbbc-ec7717576a36/scratchpad/upstream-state-machines.md is the upstream maintainer's own register (20 findings). Findings #1-#8
are fixed on an UNMERGED upstream branch (upstream/laf/state-machine-fixes, commit 1170bcbc) - do not re-report those
except to note overlap. #9-#20 are open: verify each one in your area against main (still true? reachable? exact lines?).
Then go further: the upstream register is thin on the areas below.

You are READ-ONLY. Do not edit files in the repo, do not git checkout/commit, do not run cargo build/test (another
process owns the target dir). Use grep/sed/rg/Read freely.

## What counts as an implicit state machine
State whose lifecycle is encoded in loose pieces instead of one explicit type with defined transitions:
- booleans or Options that are only valid in certain combinations
- string or int status fields, magic sentinels, "first time" flags
- retry counters, poll loops, deadlines and sleeps that stand in for a state
- ordering rules that live only in comments, call order or docs
- state persisted to disk (refmaps, sessions, traces, cursor overlay) that must agree with in-memory state
- lifecycles split across processes: CLI call -> refmap file -> next CLI call

## For each machine you find, record (in your findings file, markdown):
### <AREA>-<n>: <short name>
- Pieces: every `path:line` that holds or mutates the state
- States and transitions today (text diagram)
- Illegal / unhandled transitions reachable in practice. For each: the concrete path (call chain with lines), the input
  or race that reaches it, and a test sketch that would FAIL on main today (which stub adapter / test_support helper,
  what to assert). Mark each "CONFIRMED (traced end to end)" or "SUSPECTED (needs probe)".
- Hidden cost: fixed sleeps, repeated polls, double reads, extra platform round trips. Quote constants (ms, counts)
  and estimate the per-command cost. Mark measured vs estimate.
- Impact class (pick highest that applies): A wrong action delivered > B wrong error / wrong recovery or disposition
  > C latency > D code clarity only.
- Proposed explicit form: enum/typestate name, variants, single transition fn, which illegal states become
  unrepresentable; files touched; rough size (S <100 lines, M <400, L more). Note if a behavior fix can be split from
  a pure refactor.
- Overlap: upstream register # if any; files a perf/latency change would also touch.

Be thorough and skeptical: a machine with no reachable illegal transition and no cost is still worth one short entry
(impact D) so the ledger is complete, but spend your depth on A/B findings. Prefer fewer, verified findings over
many guesses. Respect the repo rules when proposing fixes: app-agnostic, 400-line files, no inline comments, one type
per file, no unwrap/expect/panic outside tests, dependency inversion (core never imports platform crates).

## Output
Write your full findings to /tmp/claude-1000/-home-george--t3-scratch-2026-11-07-goal-find-every-implicit-state-1f4e3925/11164cc5-bf9f-4a7c-bbbc-ec7717576a36/scratchpad/findings/<AREA>.md. Then reply with a compact summary: one line per finding
(id, name, impact class, confirmed/suspected, one-clause illegal transition, proposed fix size). Max ~40 lines.

## Prior art (mandatory, added by George)
For every A/B/C finding add a "Prior art" line. Read-only searches only, never comment/react/file on vercel-labs:
- `gh issue list --repo vercel-labs/agent-desktop --state all --search "<keywords>" --limit 50`
- `gh pr list --repo vercel-labs/agent-desktop --state all --search "<keywords>" --limit 50`
- `gh pr list --repo gshost1/agent-desktop --state all --search "<keywords>"` and `git -C /home/george/Projects/agent-desktop branch -r` (perf/ branches from the Mac speed session)
- `git -C /home/george/Projects/agent-desktop log upstream/main --oneline -S<symbol>` or `--grep=<keyword>` for an already-merged fix
Try 2-3 keyword sets per finding (symptom, error code, file/function names). Record the link(s) found, or
"none found" plus the exact searches run. If upstream/main already fixed it, mark the finding DROPPED.
