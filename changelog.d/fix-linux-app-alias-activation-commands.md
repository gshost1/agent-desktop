### Bug Fixes

* **linux:** an app started through one of its desktop entry's actions (`gnome-text-editor --new-window`) or by session-bus activation (`gnome-text-editor --gapplication-service`, how GNOME Shell starts a `DBusActivatable` app) now answers to the entry's `Name` and id, so `--app "Text Editor"`, `list-apps --app` and a scoped `wait --event window-opened` find it; before, it answered only to its executable name and a scoped wait timed out with an empty baseline ([#223](https://github.com/gshost1/agent-desktop/pull/223))
