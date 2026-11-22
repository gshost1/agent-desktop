use agent_desktop_core::{
    AdapterError, AppError, DeliverySemantics, ErrorCode, PermissionReport, refs::validate_ref_id,
};

use crate::cli::Commands;

mod command_effect;
mod connection_hint;
mod permission_read;
mod preflight_budget;

pub(crate) use command_effect::CommandEffect;
pub(crate) use connection_hint::prepare_connections;
pub(crate) use permission_read::read_permission_report;
pub(crate) use preflight_budget::preflight_deadline;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermissionNeed {
    None,
    Accessibility,
    ScreenRecording,
    AccessibilityAndScreenRecording,
}

pub(crate) fn policy_for(cmd: &Commands) -> PermissionNeed {
    use PermissionNeed::{Accessibility, AccessibilityAndScreenRecording, None, ScreenRecording};
    match cmd {
        Commands::Version
        | Commands::Skills(_)
        | Commands::Session(_)
        | Commands::CursorOverlay(_)
        | Commands::Trace(_) => None,
        Commands::Status | Commands::Permissions(_) => None,
        Commands::ListWindows(_) | Commands::ListDisplays | Commands::ListApps(_) => None,
        Commands::ClipboardGet(_) | Commands::ClipboardSet(_) | Commands::ClipboardClear => None,
        Commands::Batch(_) => None,

        Commands::Snapshot(_)
        | Commands::Find(_)
        | Commands::ListSurfaces(_)
        | Commands::Wait(_)
        | Commands::ListNotifications(_) => Accessibility,

        Commands::Screenshot(a) if a.scope.app.is_some() || a.scope.window_id.is_some() => {
            AccessibilityAndScreenRecording
        }
        Commands::Screenshot(_) => ScreenRecording,

        Commands::Get(_) | Commands::Is(_) => Accessibility,

        Commands::Click(_)
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
        | Commands::ScrollTo(_) => Accessibility,

        Commands::Type(_) => Accessibility,
        Commands::Focus(_) => Accessibility,
        Commands::Press(_) | Commands::KeyDown(_) | Commands::KeyUp(_) => Accessibility,
        Commands::Hover(_)
        | Commands::Drag(_)
        | Commands::MouseMove(_)
        | Commands::MouseClick(_)
        | Commands::MouseDown(_)
        | Commands::MouseUp(_)
        | Commands::MouseWheel(_) => Accessibility,

        Commands::Launch(_)
        | Commands::CloseApp(_)
        | Commands::FocusWindow(_)
        | Commands::OpenSystemSurface(_)
        | Commands::OpenUrl(_)
        | Commands::ResizeWindow(_)
        | Commands::MoveWindow(_)
        | Commands::Minimize(_)
        | Commands::Maximize(_)
        | Commands::Restore(_)
        | Commands::DismissNotification(_)
        | Commands::DismissAllNotifications(_)
        | Commands::NotificationAction(_) => Accessibility,
    }
}

pub(crate) fn preflight(cmd: &Commands, report: &PermissionReport) -> Result<(), AppError> {
    validate_args(cmd)?;
    let permission = policy_for(cmd);
    if requires_accessibility(permission) && report.accessibility_denied() {
        let err = AdapterError::new(
            ErrorCode::PermDenied,
            "Accessibility permission not granted",
        )
        .with_suggestion(
            report
                .accessibility_suggestion()
                .unwrap_or("Grant Accessibility permission and retry"),
        )
        .with_disposition(DeliverySemantics::not_delivered());
        return Err(AppError::Adapter(err));
    }
    if requires_screen_recording(permission) && report.screen_recording_denied() {
        let err = AdapterError::new(
            ErrorCode::PermDenied,
            "Screen Recording permission not granted",
        )
        .with_suggestion(
            report
                .screen_recording_suggestion()
                .unwrap_or("Grant Screen Recording permission and retry"),
        )
        .with_disposition(DeliverySemantics::not_delivered());
        return Err(AppError::Adapter(err));
    }
    Ok(())
}

fn agent_id_missing(cmd: &Commands, multi_agent: bool, agent_id: Option<&str>) -> bool {
    multi_agent
        && agent_id.is_none()
        && cmd.is_mutating()
        && policy_for(cmd) == PermissionNeed::Accessibility
}

pub(crate) fn preflight_context(
    cmd: &Commands,
    context: &agent_desktop_core::context::CommandContext,
) -> Result<(), AppError> {
    if agent_id_missing(cmd, context.multi_agent(), context.agent_id()) {
        return Err(AppError::invalid_input_with_suggestion(
            "Multi-agent sessions require --agent-id for desktop actions",
            "Set --agent-id or AGENT_DESKTOP_AGENT_ID before running the command.",
        ));
    }
    Ok(())
}

pub(crate) fn requires_full_permission_report(cmd: &Commands) -> bool {
    matches!(
        cmd,
        Commands::Status | Commands::Permissions(_) | Commands::Batch(_)
    )
}

pub(crate) fn requires_permission_report(cmd: &Commands) -> bool {
    policy_for(cmd) != PermissionNeed::None
        || matches!(
            cmd,
            Commands::Status | Commands::Permissions(_) | Commands::Batch(_)
        )
}

fn validate_args(cmd: &Commands) -> Result<(), AppError> {
    match cmd {
        Commands::Snapshot(args) => {
            if let Some(root) = &args.root {
                if args.surface != crate::cli_args::Surface::Window {
                    return Err(AppError::invalid_input(
                        "--root cannot be combined with --surface",
                    ));
                }
                validate_ref_id(root)?;
            }
        }
        Commands::Get(args) => {
            validate_ref_id(&args.ref_id)?;
        }
        Commands::Is(args) => {
            validate_ref_id(&args.ref_id)?;
        }
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
        | Commands::ScrollTo(args) => {
            validate_ref_id(&args.ref_id)?;
        }
        Commands::Type(args) => validate_ref_id(&args.ref_id)?,
        Commands::SetValue(args) => validate_ref_id(&args.ref_id)?,
        Commands::Select(args) => validate_ref_id(&args.ref_id)?,
        Commands::Scroll(args) => {
            validate_ref_id(&args.ref_id)?;
        }
        Commands::Hover(args) => {
            if let Some(ref_id) = &args.ref_id {
                validate_ref_id(ref_id)?;
            }
        }
        Commands::Drag(args) => {
            if let Some(ref_id) = &args.target.from {
                validate_ref_id(ref_id)?;
            }
            if let Some(ref_id) = &args.target.to {
                validate_ref_id(ref_id)?;
            }
        }
        Commands::Wait(args) => {
            if let Some(ref_id) = &args.mode.element {
                validate_ref_id(ref_id)?;
            }
        }
        Commands::DismissNotification(args) => {
            notification_addressing(
                args.index.is_some(),
                args.key.as_deref(),
                [&args.expected_app, &args.expected_title],
            )?;
        }
        Commands::NotificationAction(args) => {
            notification_addressing(
                true,
                args.key.as_deref(),
                [&args.expected_app, &args.expected_title],
            )?;
        }
        Commands::Find(args) => scroll_search_args(args)?,
        Commands::Screenshot(_)
        | Commands::Press(_)
        | Commands::KeyDown(_)
        | Commands::KeyUp(_)
        | Commands::MouseMove(_)
        | Commands::MouseClick(_)
        | Commands::MouseDown(_)
        | Commands::MouseUp(_)
        | Commands::MouseWheel(_)
        | Commands::Launch(_)
        | Commands::CloseApp(_)
        | Commands::ListWindows(_)
        | Commands::ListDisplays
        | Commands::ListApps(_)
        | Commands::FocusWindow(_)
        | Commands::OpenSystemSurface(_)
        | Commands::OpenUrl(_)
        | Commands::ResizeWindow(_)
        | Commands::MoveWindow(_)
        | Commands::Minimize(_)
        | Commands::Maximize(_)
        | Commands::Restore(_)
        | Commands::ListSurfaces(_)
        | Commands::ListNotifications(_)
        | Commands::DismissAllNotifications(_)
        | Commands::ClipboardGet(_)
        | Commands::ClipboardSet(_)
        | Commands::ClipboardClear
        | Commands::Status
        | Commands::Permissions(_)
        | Commands::Version
        | Commands::Batch(_)
        | Commands::Skills(_)
        | Commands::Session(_)
        | Commands::CursorOverlay(_)
        | Commands::Trace(_) => {}
    }
    Ok(())
}

/// Core's notification addressing rule, checked before any permission
/// preflight so a malformed request is refused without touching the desktop.
fn notification_addressing(
    has_index: bool,
    key: Option<&str>,
    [app, title]: [&Option<String>; 2],
) -> Result<(), AppError> {
    let identity = agent_desktop_core::NotificationIdentity {
        expected_app: app.clone(),
        expected_title: title.clone(),
    };
    Ok(agent_desktop_core::NotificationTarget::check(
        has_index, key, &identity,
    )?)
}

pub(crate) fn requires_accessibility(permission: PermissionNeed) -> bool {
    matches!(
        permission,
        PermissionNeed::Accessibility | PermissionNeed::AccessibilityAndScreenRecording
    )
}

pub(crate) fn requires_screen_recording(permission: PermissionNeed) -> bool {
    matches!(
        permission,
        PermissionNeed::ScreenRecording | PermissionNeed::AccessibilityAndScreenRecording
    )
}

/// A batch or MCP find reaches dispatch without clap's checks, so the
/// `--scroll` rules clap enforces on the command line are checked here too,
/// before any entry of a batch runs.
fn scroll_search_args(args: &crate::cli_args::FindArgs) -> Result<(), AppError> {
    let Some(direction) = args.place.scroll.as_deref() else {
        return Ok(());
    };
    if !matches!(direction, "up" | "down" | "left" | "right") {
        return Err(AppError::invalid_input(
            "find --scroll takes a direction: up, down, left or right",
        ));
    }
    let Some(root) = args.place.root.as_deref() else {
        return Err(AppError::invalid_input("find --scroll needs --root"));
    };
    validate_ref_id(root)?;
    let selection = &args.selection;
    if selection.count || selection.last || selection.nth.is_some() || selection.allow_partial {
        return Err(AppError::invalid_input(
            "find --scroll cannot take --count, --last, --nth or --allow-partial",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
