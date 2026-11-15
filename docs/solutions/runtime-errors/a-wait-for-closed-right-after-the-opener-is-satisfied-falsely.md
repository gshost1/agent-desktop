---
title: A wait for "closed" right after the opener is satisfied falsely
date: 2026-10-19
category: runtime-errors
module: crates/core/src/wait/menu_wait_phase.rs, crates/core/src/wait/menu_wait.rs, crates/core/src/commands/wait_menu.rs, src/batch/pending_seed.rs
problem_type: runtime_error
component: menu-wait
symptoms:
  - "wait --menu-closed returns at once after a click that opens a GTK4 popover, before the menu ever showed."
  - "The same sequence passes when click and wait run in one process and fails when they run as two CLI calls."
tags: [linux, macos, windows, at-spi, gtk4, menu, wait, race, state-machine]
---

## Problem

A GTK4 popover menu appears in the accessibility tree 157 to 331 ms after its opener action returns (L18-2). The live probe saw it open at 540 ms on Calculator and 688 ms on Files (L33-51). A `wait --menu-closed` that starts right after the opener polls a menu that is not yet visible, reads "closed", and returns. The wait is satisfied by the state it was meant to outlast.

## Solution

Core owns the decision for every platform. `MenuWaitPhase` (`crates/core/src/wait/menu_wait_phase.rs`) is `AwaitingOpen { until, read_closed }`, `Open`, `Closing` or `Closed`, with one transition per whole read: an open read moves any phase to `Open`; a closed read moves `Open` to `Closing` and `Closing` to `Closed`, so a menu swapped for another across one poll is not a close; it moves `AwaitingOpen` to `Closed` only when the read began at or after `until`. A second transition runs at the deadline: `AwaitingOpen` with at least one completed closed read becomes `Closed`, but only when the deadline is at or before `until`, because then the menu stayed closed for a whole wait that ended inside the grace; a deadline after `until` with no closed read that began at or after `until` is a timeout. A closed-direction wait is satisfied only in `Closed`. Each adapter's poll loop (Linux `menu_wait.rs`, macOS and Windows `system/wait.rs`) feeds its reads to the `MenuWait` core hands it and no longer compares a read with the wanted state itself.

The phase a closed wait starts in comes from evidence, not from which action ran:

- `batch` reads the menu of the waited app before the action that precedes a `wait --menu-closed` (`menu_seed`, the way it seeds `wait --event` with a pre-action baseline). A menu open before the action starts the wait in `Open`, so after a menu-item click two closed reads end it with no hold. The reading is capped at 500 ms and, on every platform, sits between the same process-identity checks the poll loop makes, so a slow read, an exited process or a reused pid leaves no seed. Every input action gets the same reading, including raw `mouse-click` and `drag`; `SeedPlan` skips commands that are not input actions (clipboard, cursor overlay, session and dismiss commands).
- Otherwise the wait starts in `AwaitingOpen` with the adapter's opener grace (`ObservationOps::menu_opener_grace`) measured from the wait's start, whatever the timeout. The default is 1.5 s (Linux, Windows): the popover delays above (157 to 999 ms after the opener returns, L18-2, L33-11, L33-51) fall inside it, because the wait itself starts only after the opener returned. macOS returns 300 ms: there a menu already reads open when a ref opener returns (-25 to -2 ms), and reads open 5 to 225 ms after a raw click on its opener (median about 45 ms, 21 runs over three apps), so a standalone `--menu-closed` with no menu holds 300 ms instead of 1.5 s.

The earlier Linux-only guard held the wait until 2 s after the last action, read from a per-process file under `$XDG_RUNTIME_DIR/agent-desktop-actions`. It was removed: raw `mouse-click` and `drag` never wrote it, so a coordinate opener got no hold, and a menu-item click held the wait for up to 2 s even with the menu already gone. The phase makes no distinction between action kinds, and the batch reading makes the menu-item case immediate.

## Prevention

- Test a wait that depends on a prior action in two processes, not one, and in a batch. A unit test with an in-process clock cannot fail for this reason.
- Do not size the hold from one toolkit. The popover delay is measured per toolkit (L18-2, L33-51).
- Treat "not yet visible" and "closed" as different states; the wait must know which one it read. Keep that decision in core so no platform can answer it differently.
