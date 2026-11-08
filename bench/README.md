# Bench rules

## Personal data: bench folders only

This bench drives George's own Mac. Every run, probe, replay, Mac verification and fix writer follows this rule:

- Use only folders the bench creates for itself under `~/agent-desktop-bench/` (for example `t3-<token>`, `bench-<token>`, `probe-<name>`). Create a fresh one per run and remove it afterwards.
- Never open, navigate to, snapshot, read, list, screenshot or act on `~/Pictures`, `~/Documents`, `~/Desktop`, `~/Downloads`, `~/Movies`, `~/Music`, the home folder, Recents, iCloud Drive, Photos, Mail, Messages, or any of George's own apps, windows or files.
- A Finder task opens its own bench folder in a front window before the model starts (`tasks.open_bench_finder_window`). Nothing relies on Finder's default window, which shows Recents or Home.
- A test that needs a large folder generates one: a `bench-*` dir under `~/agent-desktop-bench/` with a few thousand empty files.
- Act only through refs taken from a snapshot of a bench-owned window. Never reuse an element number from another snapshot, and never send blind keystrokes when the front window is not a bench window.
- If a window outside the bench opens anyway, close it without reading it, delete every snapshot, refmap, transcript output, screenshot or log that captured it, and report what opened it.

Why: on 2026-11-07 a model-free replay reused recorded element numbers on a Finder menu bar that had changed, clicked an unrelated menu item, and opened George's Pictures folder. Model runs had also started on Finder's default Recents window.

## Go to Folder and other path sheets

The Go to Folder sheet's suggestion table lists George's recent and personal folder paths. Every probe, run and writer brief that touches it:

- types or set-values a full bench path (`~/agent-desktop-bench/...`), never a partial one that the suggestions complete;
- never snapshots, finds or reads inside the suggestion table;
- never presses Return while focus is in the suggestions; it checks that the path field holds focus right before the key;
- verifies navigation only by the bench window's title (macctl `windows`), and closes any window whose title is not a bench folder at once.

Why: on 2026-11-08 a probe's Return most likely landed in the suggestion table after AXConfirm, and a Finder window ended up on a personal folder.

## Window hygiene

A probe leaves 0 windows and 0 sheets behind. Run every probe through `bash bench/probe_slot.sh <owner> <timeout_s> <output> <command...>`: it takes the desktop lock, runs the probe, then `bench/post_probe_check.py`, which fails the probe when a bench Finder window, a sheet on one, a bench TextEdit document or a personal-folder window is left, and closes the leftovers. A window with a sheet ignores its close button, so a probe dismisses the sheet (raise, Escape) before it closes the window and checks by title that it is gone.

Why: on 2026-11-08 eight bench windows with Go to Folder sheets, and one personal-folder window, piled up across probes. Their closes had failed silently.

## Brief template

Every writer or probe brief copies `bench/BRIEF_RULES.md` verbatim.

## Other rules

- Take the desktop lock (`bash bench/lockq.sh take "<owner>" 5`) for anything that drives the GUI, and release it after.
- Every cargo build or test on this Mac, in any worktree, goes through `bash bench/serial_cargo.sh <command>`: one at a time, `nice -n 20`, 2 jobs, 2 test threads, its own target dir under `~/.agent-desktop-bench-target/trees/` (a shared dir reused other worktrees' crates), never while the desktop lock is held. No per-worktree `target/`. Commits go through it too (`bash bench/serial_cargo.sh git commit ...`), because the pre-commit hook runs fmt, clippy and tests. Each tree's dir is capped at 20 GB; dirs of removed worktrees are trimmed.
- `bench/replay.py` is disabled until it checks every action's target role and name against the recorded run and stays inside bench folders.
