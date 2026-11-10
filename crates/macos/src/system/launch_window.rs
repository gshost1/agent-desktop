use agent_desktop_core::WindowInfo;

/// What one read of a launched application's windows shows. `Several` holds
/// the ids of windows the launch cannot choose between: more than one is
/// visible and none is the single focused one.
#[derive(Debug, Clone)]
pub(crate) enum LaunchWindow {
    Shown(WindowInfo),
    Absent,
    Several(Vec<String>),
}

impl LaunchWindow {
    /// The only window, else the only focused one. A launch that restores
    /// several windows in the background has no window it can claim, which
    /// is not a failure of the launch.
    pub(crate) fn pick(mut windows: Vec<WindowInfo>) -> Self {
        if windows.len() == 1 {
            return Self::Shown(windows.remove(0));
        }
        let mut focused: Vec<WindowInfo> = windows
            .iter()
            .filter(|window| window.state.is_focused)
            .cloned()
            .collect();
        if focused.len() == 1 {
            return Self::Shown(focused.remove(0));
        }
        if windows.is_empty() {
            return Self::Absent;
        }
        Self::Several(windows.into_iter().map(|window| window.id).collect())
    }

    /// The window the result reports and the note that explains a missing
    /// one.
    pub(crate) fn into_parts(self) -> (Option<WindowInfo>, Option<String>) {
        match self {
            Self::Shown(window) => (Some(window), None),
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

#[cfg(test)]
#[path = "launch_window_tests.rs"]
mod tests;
