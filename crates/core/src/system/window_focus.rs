use std::time::Duration;

use crate::{
    AppError, Deadline, ErrorCode, InteractionLease, PlatformAdapter, WindowFilter, WindowInfo,
};

/// Bounds the informational read after a confirmed focus, so a hung
/// accessibility read cannot spend the rest of the lease.
const FOLLOW_UP_READ_BUDGET_MS: u64 = 750;

/// Focuses `window`, then reports the window one follow-up read sees holding
/// focus. The adapter's `Ok` is the verdict: `focus_window` returns only after
/// the window is confirmed focused. The read is informational and is accepted
/// only when it names the target or another window of the target's process,
/// such as a modal dialog. `None` means the read failed, saw no window, saw a
/// window of another process, or had no time left.
pub(crate) fn focus_and_observe(
    adapter: &dyn PlatformAdapter,
    window: &WindowInfo,
    lease: &InteractionLease,
) -> Result<Option<WindowInfo>, AppError> {
    adapter.focus_window(window, lease)?;
    let deadline = lease
        .deadline()
        .capped(Duration::from_millis(FOLLOW_UP_READ_BUDGET_MS));
    Ok(follow_up_read(adapter, window, deadline))
}

/// The target as the adapter confirmed it. Fields other than the focus come
/// from the inventory read before the focus.
pub(crate) fn confirmed_target(window: WindowInfo) -> WindowInfo {
    let mut confirmed = window;
    confirmed.state.is_focused = true;
    confirmed
}

fn same_process(target: &WindowInfo, seen: &WindowInfo) -> bool {
    let same_instance = match (&target.process_instance, &seen.process_instance) {
        (Some(target), Some(seen)) => target == seen,
        _ => true,
    };
    seen.id == target.id || (seen.pid == target.pid && same_instance)
}

fn follow_up_read(
    adapter: &dyn PlatformAdapter,
    target: &WindowInfo,
    deadline: Deadline,
) -> Option<WindowInfo> {
    if deadline.is_expired() {
        return None;
    }
    match adapter.focused_window(deadline) {
        Err(err) if err.code == ErrorCode::PlatformNotSupported => adapter
            .list_windows(
                &WindowFilter {
                    focused_only: true,
                    app: Some(target.app.clone()),
                },
                deadline,
            )
            .ok()?
            .into_iter()
            .find(|seen| same_process(target, seen)),
        read => read
            .ok()
            .flatten()
            .filter(|seen| same_process(target, seen)),
    }
}
