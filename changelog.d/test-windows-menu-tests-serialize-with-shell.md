### Testing

* Windows live tests that open the fixture's context menu now hold the shell-surface test lock, so a concurrent test that opens Start, Action Center or the Windows 11 Notepad flyout can no longer take the foreground and dismiss the menu mid-test (red `Test (Windows ARM64)` on main 92094e8ea, green on rerun).
