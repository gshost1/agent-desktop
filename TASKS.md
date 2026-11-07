# State-machine audit: tasks and ledger

Branch `docs/state-machine-ledger`. Pushed at checkpoints, never opened as a PR.

## Setup (2026-11-07, laptop)

- [x] Clone `gshost1/agent-desktop`, `upstream` remote added, `main` = `upstream/main` = `b43ac4f6`.
- [x] `gh repo set-default gshost1/agent-desktop`; `gh` logged in as gshost1 with admin/push.
- [x] Toolchain 1.89.0 (`rust-toolchain.toml`) active via `~/.cargo/bin`; `core.hooksPath = .githooks`.
- [x] Writing worktrees live in `.agents/worktrees/<name>` (ignored through `.git/info/exclude`).
- [ ] Linux E2E: **not runnable on this laptop yet.** No logged-in desktop session for george (only the gdm
      greeter), which is fine because the harness starts private Xvfb/mutter sessions. Missing prerequisites from
      `runner/gate.py`: `xwd` (x11-apps), `xdotool`, `wmctrl`, `xclip`, `openbox`, `wl-copy`/`wl-paste`
      (wl-clipboard), `gnome-terminal`, PyQt6. Present: Xvfb, dbus, gdbus, busctl, gsettings, gnome-shell,
      PyGObject, Calculator, Text Editor, Files, Settings. Needs one `sudo apt install` from George (see report).

## Prior art

- Upstream maintainer branch `upstream/laf/state-machine-fixes` (unmerged, 2026-11-06, commits `1170bcbc`,
  `51796520`) fixes upstream register findings #1-#8 (issues #597, #598, #599, #600, #601). Its first commit had a
  `docs/state-machines.md` register with 20 findings; #9-#20 are open there. This audit does not redo #1-#8.

## Findings ledger

Ranked by user impact: A wrong action delivered > B wrong error/recovery > C latency > D clarity.

| # | Machine | Files | Illegal transitions (reachable) | Cost | Impact | Prior art | PR |
|---|---|---|---|---|---|---|---|
| | sweep in progress | | | | | | |

## PRs

| Branch | PR | Status |
|---|---|---|

## Needs Mac verification

## Needs Windows verification

## Found (outside scope)
