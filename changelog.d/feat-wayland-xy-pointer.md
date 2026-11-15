### Features

* **linux:** `mouse-click`, `mouse-move`, `mouse-wheel`, `hover --xy` and `drag --from-xy --to-xy` now work on GNOME Wayland through Mutter RemoteDesktop and ScreenCast, reported `delivered_unverified`, in the logical desktop space of `list-displays` across monitors and scales (one click 171.6 ms, min of 7); pointer input aimed at a ref still refuses with `wayland_session`, and a Wayland session without those interfaces refuses with `PLATFORM_NOT_SUPPORTED`, `wayland_interface_missing` ([#NNN](https://github.com/gshost1/agent-desktop/pull/NNN))
