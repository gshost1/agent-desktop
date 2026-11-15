### Bug Fixes

* On Windows, an open WinUI 3 menu flyout, such as the File menu of the Windows 11 Store Notepad, now counts as an open menu for `wait --menu`, `snapshot --surface menu` and the menu checks around actions. The flyout shows in a visible `Microsoft.UI.Content.PopupWindowSiteBridge` popup window that the classic menu-mode and tool-window checks never read, so `wait --menu` timed out and `snapshot --surface menu` failed `WINDOW_NOT_FOUND` while the menu was open. That broke the skill's Notepad save recipe on Windows 11.
