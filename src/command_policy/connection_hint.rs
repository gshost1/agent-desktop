//! Connections a command is about to need, started before its permission
//! preflight so their setup overlaps the permission checks. Starting one a
//! command never uses costs it time, so each command states whether it reads
//! the window inventory, and a ref command opens only the connection to the
//! process its ref was captured from.

use agent_desktop_core::{PlatformAdapter, refs::saved_ref_process::saved_ref_process};

use crate::cli::Commands;

pub(crate) fn prepare_connections(
    cmd: &Commands,
    adapter: &dyn PlatformAdapter,
    session_id: Option<&str>,
) {
    if reads_window_inventory(cmd) {
        let _ = adapter.prepare_window_inventory();
    }
    let Some(ref_id) = target_ref(cmd) else {
        return;
    };
    if let Some(pid) = saved_ref_process(ref_id, session_id) {
        let _ = adapter.prepare_application_connection(pid);
    }
}

/// The ref whose element a command reads or acts on first.
pub(crate) fn target_ref(cmd: &Commands) -> Option<&str> {
    match cmd {
        Commands::Get(args) => Some(&args.ref_id),
        Commands::Is(args) => Some(&args.ref_id),
        Commands::Click(args)
        | Commands::DoubleClick(args)
        | Commands::TripleClick(args)
        | Commands::RightClick(args)
        | Commands::Clear(args)
        | Commands::Focus(args)
        | Commands::Toggle(args)
        | Commands::Check(args)
        | Commands::Uncheck(args)
        | Commands::Expand(args)
        | Commands::Collapse(args)
        | Commands::ScrollTo(args) => Some(&args.ref_id),
        Commands::Type(args) => Some(&args.ref_id),
        Commands::SetValue(args) => Some(&args.ref_id),
        Commands::Select(args) => Some(&args.ref_id),
        Commands::Scroll(args) => Some(&args.ref_id),
        Commands::Press(args) => args.ref_id.as_deref(),
        Commands::Hover(args) => args.ref_id.as_deref(),
        Commands::Drag(args) => args.target.from.as_deref(),
        Commands::Wait(args) => args.mode.element.as_deref(),
        _ => None,
    }
}

/// Whether a command reads the window inventory or, as `press` does, posts
/// key events through the same window-server connection.
pub(crate) fn reads_window_inventory(cmd: &Commands) -> bool {
    match cmd {
        Commands::Snapshot(_)
        | Commands::Find(_)
        | Commands::Launch(_)
        | Commands::FocusWindow(_)
        | Commands::ResizeWindow(_)
        | Commands::MoveWindow(_)
        | Commands::Minimize(_)
        | Commands::Maximize(_)
        | Commands::Restore(_)
        | Commands::Press(_) => true,
        Commands::Wait(args) => args.mode.ms.is_none(),
        Commands::Screenshot(args) => args.scope.app.is_some() || args.scope.window_id.is_some(),
        Commands::Version
        | Commands::Skills(_)
        | Commands::Session(_)
        | Commands::CursorOverlay(_)
        | Commands::Trace(_)
        | Commands::Status
        | Commands::Permissions(_)
        | Commands::ListWindows(_)
        | Commands::ListDisplays
        | Commands::ListApps(_)
        | Commands::ClipboardGet(_)
        | Commands::ClipboardSet(_)
        | Commands::ClipboardClear
        | Commands::Batch(_)
        | Commands::ListSurfaces(_)
        | Commands::CloseApp(_)
        | Commands::ListNotifications(_)
        | Commands::Get(_)
        | Commands::Is(_)
        | Commands::Click(_)
        | Commands::DoubleClick(_)
        | Commands::TripleClick(_)
        | Commands::RightClick(_)
        | Commands::SetValue(_)
        | Commands::Clear(_)
        | Commands::Select(_)
        | Commands::Toggle(_)
        | Commands::Check(_)
        | Commands::Uncheck(_)
        | Commands::Expand(_)
        | Commands::Collapse(_)
        | Commands::Scroll(_)
        | Commands::ScrollTo(_)
        | Commands::Type(_)
        | Commands::Focus(_)
        | Commands::KeyDown(_)
        | Commands::KeyUp(_)
        | Commands::Hover(_)
        | Commands::Drag(_)
        | Commands::MouseMove(_)
        | Commands::MouseClick(_)
        | Commands::MouseDown(_)
        | Commands::MouseUp(_)
        | Commands::MouseWheel(_)
        | Commands::OpenSystemSurface(_)
        | Commands::OpenUrl(_)
        | Commands::DismissNotification(_)
        | Commands::DismissAllNotifications(_)
        | Commands::NotificationAction(_) => false,
    }
}

#[cfg(test)]
#[path = "connection_hint_tests.rs"]
mod tests;
