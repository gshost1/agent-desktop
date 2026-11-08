---
title: A wait for "closed" right after the opener is satisfied falsely
date: 2026-10-19
category: runtime-errors
module: crates/core/src/wait/menu_wait_phase.rs, crates/core/src/wait/menu_wait.rs, crates/core/src/commands/wait_menu.rs, src/batch/execution.rs
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

Core owns the decision for every platform. `MenuWaitPhase` (`crates/core/src/wait/menu_wait_phase.rs`) is `AwaitingOpen { until }`, `Open` or `Closed`, with one transition per whole read: an open read moves any phase to `Open`; a closed read moves `Open` to `Closed`, and moves `AwaitingOpen` to `Closed` only when the read began at or after `until`. A closed-direction wait is satisfied only in `Closed`. Each adapter's poll loop (Linux `menu_wait.rs`, macOS and Windows `system/wait.rs`) feeds its reads to the `MenuWait` core hands it and no longer compares a read with the wanted state itself.

The phase a closed wait starts in comes from evidence, not from which action ran:

- `batch` reads the menu of the waited app before the action that precedes a `wait --menu-closed` (`menu_seed`, the way it seeds `wait --event` with a pre-action baseline). A menu open before the action starts the wait in `Open`, so after a menu-item click the first closed read ends it with no hold.
- Otherwise the wait starts in `AwaitingOpen` with a 1 s opener grace measured from the wait's start, cut to half the timeout when that is shorter. The popover delays above (157 to 999 ms after the opener returns, L18-2, L33-11, L33-51) fall inside it, because the wait itself starts only after the opener returned.

The earlier Linux-only guard held the wait until 2 s after the last action, read from a per-process file under `$XDG_RUNTIME_DIR/agent-desktop-actions`. It was removed: raw `mouse-click` and `drag` never wrote it, so a coordinate opener got no hold, and a menu-item click held the wait for up to 2 s even with the menu already gone. The phase makes no distinction between action kinds, and the batch reading makes the menu-item case immediate.

## Prevention

- Test a wait that depends on a prior action in two processes, not one, and in a batch. A unit test with an in-process clock cannot fail for this reason.
- Do not size the hold from one toolkit. The popover delay is measured per toolkit (L18-2, L33-51).
- Treat "not yet visible" and "closed" as different states; the wait must know which one it read. Keep that decision in core so no platform can answer it differently.
