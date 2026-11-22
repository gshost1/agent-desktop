### Bug Fixes

* Linux: `launch` prefers a new descendant with an accessible window over a windowless descendant registered first, such as a wrapper's helper or a restarting process, and reports `AMBIGUOUS_TARGET` when two descendants have windows. When no descendant has a window, a windowed process matching the launched executable outside the spawned process tree can own the launch. Windowless candidates wait until near the launch deadline, with descendants preferred over outside identity matches.
* Linux: `launch` waits the full launch deadline for a process that maps a window but never joins the accessibility bus, instead of returning after about 7 seconds.
* Linux: the off-bus `launch` suggestion names the largest normal window, skipping splash screens and any window that closes mid-read, and suggests `wait --event window-opened` when the title cannot be matched reliably. It no longer creates X11 atoms.

### Performance

* Linux: while `launch` waits on a new windowless descendant, such as a tray or background process, it checks the root's children before a full window read, reuses a windowless read while those children stay the same, and remembers a process instance's window once seen.
