### Bug Fixes

* macOS: a renamed or wrapped CLI binary now draws the cursor overlay; the renderer is no longer refused because the executable is not named `agent-desktop`. A process that may not start the renderer refuses with `PLATFORM_NOT_SUPPORTED`, so the CLI reports `rendered: false` instead of claiming an overlay it never started, and an FFI host draws no cursor.
