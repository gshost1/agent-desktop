//! The windows a process had open before an action, so a wait that follows
//! the action can tell the windows it opened from the ones already there.

use std::collections::BTreeSet;
use std::time::Duration;

use crate::{
    AdapterError, CommandContext, Deadline, PlatformAdapter, ProcessIdentity, RefEntry,
    WindowFilter, WindowInfo,
};

/// The longest an action spends listing its process's windows before it is
/// sent. A slower listing leaves the wait on the ref's own window.
pub(crate) const WINDOW_BASELINE_BUDGET: Duration = Duration::from_millis(500);

/// The least an action must have left for the listing to run, so the listing
/// can take at most half of what remains and never pushes the dispatch past
/// the action's deadline.
pub(crate) const WINDOW_BASELINE_RESERVE: Duration = Duration::from_millis(1_000);

/// The windows of one process instance that were open before an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowBaseline {
    app: String,
    process: ProcessIdentity,
    open: BTreeSet<String>,
}

impl WindowBaseline {
    /// The baseline an appear wait after an action on `entry` follows, taken
    /// before the action is sent within the action's own `deadline`. `None`
    /// without such a wait, for a ref the wait does not follow, when less
    /// than [`WINDOW_BASELINE_RESERVE`] is left, or when the windows cannot be
    /// listed.
    pub(crate) fn before_action(
        adapter: &dyn PlatformAdapter,
        entry: &RefEntry,
        context: &CommandContext,
        deadline: Deadline,
    ) -> Option<Self> {
        if context.wait_selector().is_none_or(|wait| wait.gone)
            || !crate::wait::ref_wait::follows_windows(adapter, entry)
            || deadline.remaining() < WINDOW_BASELINE_RESERVE
        {
            return None;
        }
        Self::capture(
            adapter,
            entry.source.source_app.as_deref()?,
            entry.process.identity()?,
            deadline,
        )
    }

    /// Lists the process's windows, or `None` when the platform cannot list
    /// them by process instance in time.
    pub(crate) fn capture(
        adapter: &dyn PlatformAdapter,
        app: &str,
        process: ProcessIdentity,
        deadline: Deadline,
    ) -> Option<Self> {
        let windows = process_windows(
            adapter,
            app,
            &process,
            deadline.capped(WINDOW_BASELINE_BUDGET),
        )
        .ok()?;
        Some(Self {
            app: app.to_owned(),
            process,
            open: windows.into_iter().map(|window| window.id).collect(),
        })
    }

    /// The process's windows that were not open before the action, in the
    /// order the platform lists them.
    pub(crate) fn opened_since(
        &self,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(
            process_windows(adapter, &self.app, &self.process, deadline)?
                .into_iter()
                .filter(|window| !self.open.contains(&window.id))
                .collect(),
        )
    }

    /// The windows an application-level command such as a menu item acts in:
    /// the window it acts in, where a sheet it opens attaches, then the
    /// windows it opened since the action.
    pub(crate) fn acted_on(
        &self,
        adapter: &dyn PlatformAdapter,
        deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        let windows = process_windows(adapter, &self.app, &self.process, deadline)?;
        Ok(acted_on_among(adapter, windows, Some(&self.open), deadline))
    }
}

/// The window an application-level command on `entry` acts in when no
/// baseline was taken before it. Without one, the windows it opened cannot
/// be told from the ones already open, so none of those is read.
pub(crate) fn acted_on_without_baseline(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let (Some(app), Some(process)) = (entry.source.source_app.as_deref(), entry.process.identity())
    else {
        return Ok(Vec::new());
    };
    let windows = process_windows(adapter, app, &process, deadline)?;
    Ok(acted_on_among(adapter, windows, None, deadline))
}

/// The process's focused window, or its front window when none is focused,
/// as for an application in the background, followed by every other window
/// not in `open_before`.
fn acted_on_among(
    adapter: &dyn PlatformAdapter,
    windows: Vec<WindowInfo>,
    open_before: Option<&BTreeSet<String>>,
    deadline: Deadline,
) -> Vec<WindowInfo> {
    let (mut acted, mut rest): (Vec<_>, Vec<_>) = windows
        .into_iter()
        .partition(|window| window.state.is_focused);
    if acted.is_empty()
        && let Some(front) = front_standard_window(adapter, &rest, deadline)
    {
        acted.push(rest.remove(front));
    }
    let opened = rest
        .into_iter()
        .filter(|window| open_before.is_some_and(|open| !open.contains(&window.id)));
    acted.into_iter().chain(opened).collect()
}

/// The front window an application command acts in when no window is
/// focused. Among the accessible, unminimized windows the platform does not
/// report hidden, the ones the adapter reports as auxiliary, such as a
/// floating inspector, are set aside as an `--app` window choice sets them
/// aside, since a command acts in a document window or dialog; a set that is
/// all auxiliary or has none is taken as it stands.
fn front_standard_window(
    adapter: &dyn PlatformAdapter,
    windows: &[WindowInfo],
    deadline: Deadline,
) -> Option<usize> {
    let shown: Vec<WindowInfo> = windows
        .iter()
        .filter(|window| {
            window.state.accessible
                && window.state.minimized != Some(true)
                && window.state.visible != Some(false)
        })
        .cloned()
        .collect();
    let candidates = match shown.len() {
        0 | 1 => shown,
        _ => crate::system::window_lookup::standard_windows(adapter, shown.clone(), deadline)
            .unwrap_or(shown),
    };
    let front = front_window(&candidates)?;
    windows.iter().position(|window| window.id == front.id)
}

/// The first window the platform lists as visible, or failing that the first
/// whose visibility it does not report. A platform that lists a process's
/// windows front to back names its front window first.
fn front_window(windows: &[WindowInfo]) -> Option<&WindowInfo> {
    windows
        .iter()
        .find(|window| window.state.visible == Some(true))
        .or_else(|| windows.iter().find(|window| window.state.visible.is_none()))
}

/// Every window of one process instance. A window whose process instance the
/// platform does not report is left out, so a reused process id never lends
/// its windows to the wait.
pub(crate) fn process_windows(
    adapter: &dyn PlatformAdapter,
    app: &str,
    process: &ProcessIdentity,
    deadline: Deadline,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let filter = WindowFilter {
        focused_only: false,
        app: Some(app.to_owned()),
    };
    Ok(adapter
        .list_windows(&filter, deadline)?
        .into_iter()
        .filter(|window| {
            window.pid == process.pid
                && window.process_instance.as_deref() == Some(process.instance.as_str())
        })
        .collect())
}

#[cfg(test)]
#[path = "window_baseline_tests.rs"]
mod tests;
