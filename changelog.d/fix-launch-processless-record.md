### Bug Fixes

* On macOS, `launch` goes on past an AppKit application record that has no process (pid -1 or 0), as `list-apps` does, instead of retrying until it fails `TIMEOUT`. When no running app matched, any skipped record counted as an incomplete inventory, and a record with no process never resolves while the launch waits. A record skipped for any other reason, which could be the app being launched, still fails the read. (PRLINK)

### Documentation

* The 1.4.0 entry for [#116](https://github.com/gshost1/agent-desktop/pull/116) said `close-app` "now returns within 3 ms" of the app's exit. That bound is not established: the exit was timed by a separate kqueue watcher whose own scheduling is in each number, and on System Settings `close-app` returned before the watcher saw the exit (lags down to -14.8 ms, and -21.8 ms before the change). What was measured is a median exit-to-return lag of 0.8-0.9 ms on Calculator, the SwiftUI fixture and Weather, down from 1.3-12.5 ms, with a largest lag of 2.4 ms over 10 runs per app.
