### Bug Fixes

* On macOS, clamped or ignored moves and resizes now report the requested and observed geometry. Maximize can still time out, as can move or resize if the deadline expires before any frame is readable and before the readback budget ends.
