use agent_desktop_core::WindowInfo;

/// What one read of a launched application's windows shows. `Several` holds
/// the ids of windows the launch cannot choose between: more than one is
/// visible and none is the single focused one.
#[derive(Debug, Clone)]
pub(crate) enum LaunchWindow {
    Shown(WindowInfo),
    /// The window the launch would claim, read before the window server put
    /// it on screen: not visible, not minimized and without a size. Its
    /// bounds and visibility are not yet the window's, so the wait reads
    /// again; a wait that ends on it still reports it.
    Forming(WindowInfo),
    Absent,
    Several(Vec<String>),
}

impl LaunchWindow {
    /// The only window, else the focused one; the inventory marks at most one
    /// window focused. A launch that restores
    /// several windows in the background has no window it can claim, which
    /// is not a failure of the launch.
    pub(crate) fn pick(mut windows: Vec<WindowInfo>) -> Self {
        if windows.len() == 1 {
            return Self::claim(windows.remove(0));
        }
        if let Some(focused) = windows.iter().position(|window| window.state.is_focused) {
            return Self::claim(windows.swap_remove(focused));
        }
        if windows.is_empty() {
            return Self::Absent;
        }
        Self::Several(windows.into_iter().map(|window| window.id).collect())
    }

    fn claim(window: WindowInfo) -> Self {
        if is_forming(&window) {
            Self::Forming(window)
        } else {
            Self::Shown(window)
        }
    }

    /// The window the result reports and the note that explains a missing
    /// one.
    pub(crate) fn into_parts(self) -> (Option<WindowInfo>, Option<String>) {
        match self {
            Self::Shown(window) | Self::Forming(window) => (Some(window), None),
            Self::Absent => (None, None),
            Self::Several(ids) => (
                None,
                Some(format!(
                    "The application shows {} windows and none of them is the single focused window, so no window is reported: {}. Run 'list-windows' and pass the window id you want.",
                    ids.len(),
                    ids.join(", ")
                )),
            ),
        }
    }
}

/// A window the window server has not placed yet reads as not visible with
/// a zero size. A real window that is off screen, such as one on another
/// Space, keeps its size, and a minimized one says so.
fn is_forming(window: &WindowInfo) -> bool {
    window.state.visible != Some(true)
        && window.state.minimized != Some(true)
        && window
            .bounds
            .as_ref()
            .is_none_or(|bounds| bounds.width <= 0.0 || bounds.height <= 0.0)
}

#[cfg(test)]
#[path = "launch_window_tests.rs"]
mod tests;
