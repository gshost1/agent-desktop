### Bug Fixes

* On Windows, `press --app` and `press --window-id` deliver to an application hosted in an `ApplicationFrameHost` frame, such as Settings, when its frame is in front. The focus gates compared the foreground and keyboard-focus windows against the frame host's pid while the window listing reported the hosted application's, so every press to a foreground hosted application failed `ACTION_FAILED` as not in front.
