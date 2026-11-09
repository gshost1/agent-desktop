use agent_desktop_core::{
    AppError, CursorOverlayConfig, CursorOverlayControl, PlatformAdapter, commands::cursor_overlay,
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::{
    cursor_overlay::CursorOverlayArgs, cursor_overlay_action::CursorOverlayAction,
};

pub(crate) fn dispatch(
    args: CursorOverlayArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let session_id = context.session_id().ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            "Cursor overlay settings require an active session",
            "Run `session start`, then pass its id with --session or AGENT_DESKTOP_SESSION.",
        )
    })?;
    match args.action {
        CursorOverlayAction::Enable(args) => {
            let multi_agent = args.multi_agent || context.multi_agent();
            let config = args.to_core()?.with_multi_agent(multi_agent);
            let agent_id = context.agent_id().map(str::to_owned);
            if let Some(agent_id) = agent_id.as_deref() {
                if args.multi_agent {
                    return Err(AppError::invalid_input(
                        "--multi-agent configures the session; omit --agent-id",
                    ));
                }
                agent_desktop_core::session::save_cursor_overlay_profile(
                    session_id,
                    agent_id,
                    config.clone(),
                )?;
                return Ok(serde_json::json!({
                    "session_id": session_id,
                    "cursor_overlay": config,
                    "agent_id": agent_id,
                }));
            }
            let mut value = cursor_overlay::execute(
                session_id,
                cursor_overlay::CursorOverlayAction::Enable(Box::new(config.clone())),
            )?;
            if !multi_agent {
                value["rendered"] = Value::from(render(adapter, session_id, &config));
            }
            Ok(value)
        }
        CursorOverlayAction::Disable => {
            let value =
                cursor_overlay::execute(session_id, cursor_overlay::CursorOverlayAction::Disable)?;
            if let Err(error) =
                adapter.update_cursor_overlay(&CursorOverlayControl::disable(session_id.to_owned()))
            {
                if !nothing_to_tear_down(&error) {
                    return Err(teardown_error(error));
                }
                super::overlay_skipped::log(&error);
            }
            Ok(value)
        }
    }
}

/// Asks the renderer to draw the cursor of a session whose manifest already
/// enables it. The overlay is presentation only, so a refusal is logged and
/// reported as `false` and the next action that draws asks again.
pub(super) fn render(
    adapter: &dyn PlatformAdapter,
    session_id: &str,
    config: &CursorOverlayConfig,
) -> bool {
    let control = CursorOverlayControl::enable(session_id.to_owned(), config.style().clone())
        .with_motion(config.motion().clone());
    match adapter.update_cursor_overlay(&control) {
        Ok(()) => true,
        Err(error) => {
            super::overlay_skipped::log(&error);
            false
        }
    }
}

fn nothing_to_tear_down(error: &agent_desktop_core::AdapterError) -> bool {
    error.code == agent_desktop_core::ErrorCode::PlatformNotSupported
}

pub(super) fn tear_down(
    adapter: &dyn PlatformAdapter,
    control: &CursorOverlayControl,
) -> Result<(), AppError> {
    match adapter.update_cursor_overlay(control) {
        Err(error) if !nothing_to_tear_down(&error) => Err(teardown_error(error)),
        _ => Ok(()),
    }
}

pub(super) fn teardown_error(mut error: agent_desktop_core::AdapterError) -> AppError {
    error.message = format!(
        "Session state was saved, but cursor teardown was not confirmed: {}",
        error.message
    );
    error
        .with_disposition(agent_desktop_core::DeliverySemantics::uncertain())
        .into()
}

#[cfg(test)]
#[path = "cursor_overlay_tests.rs"]
mod tests;
