### Bug Fixes

* Keep Linux key focus checks on the command deadline while bounding X11 activation checks to 300 ms, and preserve the last valid window listing when a focus re-read fails.
* Prevent Linux window commands on a stale dialog from targeting a same-size sibling window.
* Refuse Linux key input to dialogs with stale ACTIVE state unless X11 geometry identifies the dialog as active or a same-process transient window has an agreeing size.
* Avoid extra Linux window re-read waits for persistent frame identity mismatches; re-read only when an unclaimed native window indicates a window is mapping.
