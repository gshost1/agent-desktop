use std::time::Duration;

use super::focus_corroboration::FocusCorroboration;
use crate::{
    AdapterError, AppError, Deadline, ErrorCode, InteractionLease, PlatformAdapter, WindowFilter,
    WindowInfo,
};

/// The slice of the lease deadline withheld from the adapter so core's one
/// corroborating read still has time after a late adapter confirmation.
pub(crate) const CORROBORATION_RESERVE_MS: u64 = 150;

/// Focuses `window`, then corroborates it with one read. The adapter confirms
/// focus itself, as the `focus_window` contract requires, within the lease
/// deadline minus [`CORROBORATION_RESERVE_MS`]; a read that disagrees or
/// cannot run reports the focus `delivered_unverified`.
pub(crate) fn focus_and_confirm(
    adapter: &dyn PlatformAdapter,
    window: &WindowInfo,
    lease: &InteractionLease,
) -> Result<WindowInfo, AppError> {
    let deadline = lease.deadline();
    adapter.focus_window(window, &lease.with_deadline(adapter_deadline(deadline)))?;
    let read = corroborating_read(adapter, &window.app, deadline);
    FocusCorroboration::judge(&window.id, read).into_result()
}

fn adapter_deadline(deadline: Deadline) -> Deadline {
    let remaining = deadline.remaining();
    let reserve = Duration::from_millis(CORROBORATION_RESERVE_MS).min(remaining / 4);
    deadline.capped(remaining.saturating_sub(reserve))
}

fn corroborating_read(
    adapter: &dyn PlatformAdapter,
    app: &str,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    deadline.ensure_remaining()?;
    match adapter.focused_window(deadline) {
        Err(err) if err.code == ErrorCode::PlatformNotSupported => adapter
            .list_windows(
                &WindowFilter {
                    focused_only: true,
                    app: Some(app.to_string()),
                },
                deadline,
            )
            .map(|windows| windows.into_iter().next()),
        read => read,
    }
}
