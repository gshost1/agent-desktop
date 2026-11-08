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

## Other rules

- Take the desktop lock (`bash bench/lockq.sh take "<owner>" 5`) for anything that drives the GUI, and release it after.
- Every cargo build or test on this Mac, in any worktree, goes through `bash bench/serial_cargo.sh <command>`: one at a time, `nice -n 20`, 2 jobs, 2 test threads, the shared `~/.agent-desktop-bench-target`, never while the desktop lock is held. No per-worktree `target/`. Commits go through it too (`bash bench/serial_cargo.sh git commit ...`), because the pre-commit hook runs fmt, clippy and tests. Keep bench build output under 10 GB.
- `bench/replay.py` is disabled until it checks every action's target role and name against the recorded run and stays inside bench folders.
