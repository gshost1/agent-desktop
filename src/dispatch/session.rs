use agent_desktop_core::{
    AppError, CursorOverlayConfig, CursorOverlayControl, PlatformAdapter, commands::session,
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::session::{SessionAction, SessionArgs};

pub(crate) fn dispatch(
    args: SessionArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    match args.action {
        SessionAction::Start(s) => {
            let cursor_overlay = if s.cursor {
                CursorOverlayConfig::enabled(None, 6)?.with_multi_agent(s.multi_agent)
            } else {
                CursorOverlayConfig::default()
            };
            let mut value =
                session::start(s.name, s.no_trace, s.screenshots, cursor_overlay.clone())?;
            if s.cursor {
                value["rendered"] = Value::Bool(render_cursor(adapter, &value, &cursor_overlay));
            }
            Ok(value)
        }
        SessionAction::End(e) => {
            let id = resolve_end_session_id(e.id, context.session_id())?;
            let value = session::execute(session::SessionAction::End { id: id.clone() })?;
            super::cursor_overlay::tear_down(adapter, &CursorOverlayControl::disable(id))?;
            Ok(value)
        }
        SessionAction::List => session::execute(session::SessionAction::List),
        SessionAction::Gc(g) => session::execute(session::SessionAction::Gc {
            older_than_secs: g.older_than,
            ended_only: g.ended,
        }),
    }
}

/// Draws the cursor of a session whose manifest already enables it. The
/// overlay is presentation only, so a renderer that cannot draw leaves the
/// session started and reports `rendered: false`; the next action that needs
/// the cursor asks for the renderer again.
fn render_cursor(
    adapter: &dyn PlatformAdapter,
    value: &Value,
    config: &CursorOverlayConfig,
) -> bool {
    let Some(id) = value.get("session_id").and_then(Value::as_str) else {
        return false;
    };
    if config.is_multi_agent() {
        return false;
    }
    let control = CursorOverlayControl::enable(id.to_owned(), config.style().clone())
        .with_motion(config.motion().clone());
    match adapter.update_cursor_overlay(&control) {
        Ok(()) => true,
        Err(error) => {
            super::overlay_skipped::log(&error);
            false
        }
    }
}

fn resolve_end_session_id(
    explicit: Option<String>,
    active: Option<&str>,
) -> Result<String, AppError> {
    explicit
        .or_else(|| active.map(str::to_string))
        .ok_or_else(|| {
            AppError::invalid_input_with_suggestion(
                "No session id was supplied and no active session scope is configured",
                "Pass `session end <id>`, global `--session <id>`, or AGENT_DESKTOP_SESSION",
            )
        })
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
