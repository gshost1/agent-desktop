### Bug Fixes

* On Windows, `press --app` and `press --window-id` deliver to an application hosted in an `ApplicationFrameHost` frame, such as Settings, when its frame is in front. `list-apps` now names such a frame's hosted application under its own pid, as `list-windows` already did, instead of `ApplicationFrameHost.exe`, so `press --app SystemSettings.exe` finds it; and the focus gates read the foreground and keyboard-focus windows under the same identity, so a press to a foreground hosted application no longer fails `ACTION_FAILED` as not in front.
