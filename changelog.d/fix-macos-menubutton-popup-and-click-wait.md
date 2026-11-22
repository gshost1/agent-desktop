### Bug Fixes

* Treat a macOS window its app ordered out (off screen, gone from AXWindows, on no Space) as closed so post-click waits follow the new window, while windows on another Space, behind a full-screen Space or mid full-screen transition stay present, and bound the Space read by the command deadline.
