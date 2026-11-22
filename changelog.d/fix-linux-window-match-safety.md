### Bug Fixes

* Keep Linux key focus checks on the command deadline while bounding X11 activation checks to 300 ms, and preserve the last valid window listing when a focus re-read fails.
* Prevent Linux window commands on a stale dialog from targeting a same-size sibling window.
* Refuse Linux key input to dialogs with stale ACTIVE state unless X11 geometry identifies the dialog as active or a same-process transient window has identity proof and a unique listed size.
* Focus Linux dialogs with different accessibility and native titles through coherent frame geometry, while refusing unproved or competing matches and preserving the named-window focus fast path.
* Use coherent Linux window matching for capture, hidden-window filtering, and focus candidates so geometry cannot claim a named frame's client or share one client between competing frames.
* Avoid extra Linux window re-read waits for persistent frame identity mismatches; re-read only when an unclaimed native window indicates a window is mapping.
