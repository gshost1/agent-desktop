---
module: agent-desktop-macos
date: 2026-11-07
problem_type: runtime_error
tags: [visibility, clipping, sheet, actionability]
---

# A sheet wider than its window reported its own controls offscreen

A TextEdit Save sheet is 880 points wide and hangs from a 586-point document
window. Every clipping ancestor was intersected into the element's viewport,
the sheet's frame and the window's frame alike, so the sheet's right side fell
outside the result. Its Save button and sidebar cells reported `offscreen`, and
actionability refused them with "visible (live offscreen state is true)". In
bench runs that refusal cost over 100 s of retries, window moves and resizes.

A sheet or popover is a window of its own; the window it hangs from does not
clip it. `draws_own_surface` in `tree/element_bounds.rs` names those roles. In
snapshot traversal the sheet's frame replaces the inherited viewport, in the
live ancestor walk it ends the list of clips (`viewport_from_clips`), and in the
pointer hit test it is the last clip applied. Clips inside the sheet, such as a
scroll area, still narrow it, and a sheet with unknown geometry leaves the
viewport unknown rather than falling back to the window.
