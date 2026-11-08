use crate::{
    AppError, Deadline, ErrorCode, InteractionLease, PlatformAdapter, WindowFilter, WindowInfo,
};

/// Focuses `window` and reports which window a follow-up read sees holding
/// focus. The adapter's `Ok` is the verdict: `focus_window` returns only after
/// the window is confirmed focused. The read is informational, so a modal
/// dialog it sees is reported, and a read that fails, sees nothing or has no
/// time left reports the target as the adapter confirmed it.
pub(crate) fn focus_and_confirm(
    adapter: &dyn PlatformAdapter,
    window: &WindowInfo,
    lease: &InteractionLease,
) -> Result<WindowInfo, AppError> {
    adapter.focus_window(window, lease)?;
    Ok(follow_up_read(adapter, &window.app, lease.deadline())
        .unwrap_or_else(|| confirmed_target(window)))
}

fn confirmed_target(window: &WindowInfo) -> WindowInfo {
    let mut confirmed = window.clone();
    confirmed.state.is_focused = true;
    confirmed
}

fn follow_up_read(
    adapter: &dyn PlatformAdapter,
    app: &str,
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
                    app: Some(app.to_string()),
                },
                deadline,
            )
            .ok()?
            .into_iter()
            .next(),
        read => read.ok().flatten(),
    }
}
