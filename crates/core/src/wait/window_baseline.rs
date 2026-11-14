//! The windows a process had open before an action, so a wait that follows
//! the action can tell the windows it opened from the ones already there.

use std::collections::BTreeSet;
use std::time::Duration;

use crate::{
    AdapterError, Deadline, PlatformAdapter, ProcessIdentity, RefEntry, WaitSelector, WindowFilter,
    WindowInfo,
};

/// The longest an action spends listing its process's windows before it is
/// sent. A slower listing leaves the wait on the ref's own window.
pub(crate) const WINDOW_BASELINE_BUDGET: Duration = Duration::from_millis(500);

/// The windows of one process instance that were open before an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowBaseline {
    app: String,
    process: ProcessIdentity,
    open: BTreeSet<String>,
}

impl WindowBaseline {
    /// The baseline an appear wait after an action on `entry` follows, taken
    /// before the action is sent. `None` without such a wait, for a ref that
    /// already observes its whole owner, or when the windows cannot be listed.
    pub(crate) fn before_action(
        adapter: &dyn PlatformAdapter,
        entry: &RefEntry,
        wait: Option<&WaitSelector>,
    ) -> Option<Self> {
        if wait.is_none_or(|wait| wait.gone)
            || adapter.is_application_surface(entry.source.source_surface)
            || entry.source.is_process_owned()
        {
            return None;
        }
        let deadline = Deadline::from_duration(WINDOW_BASELINE_BUDGET).ok()?;
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
