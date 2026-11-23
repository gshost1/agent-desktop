### Bug Fixes

* macOS: a renamed or wrapped CLI binary now draws the cursor overlay; the renderer is no longer refused because the executable is not named `agent-desktop`. A process that cannot start the renderer, such as an FFI host, now reports `rendered: false` instead of claiming an overlay it never started.
