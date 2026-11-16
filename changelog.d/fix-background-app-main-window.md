### Bug Fixes

* macOS: `snapshot --app`, `find --app` and the other `--app` commands on an app in the background with two or more windows read the window macOS names the app's main window, reported in `data.window`, instead of failing with `AMBIGUOUS_TARGET`
