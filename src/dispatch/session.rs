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
            let cursor_overlay = s
                .cursor
                .then(|| CursorOverlayConfig::enabled(None, 6))
                .transpose()?
                .map(|config| config.with_multi_agent(s.multi_agent));
            let (manifest, mut value) =
                session::start(s.name, s.no_trace, s.screenshots, cursor_overlay.clone())?;
            if let Some(config) = cursor_overlay {
                value["rendered"] = Value::from(
                    !config.is_multi_agent()
                        && super::cursor_overlay::render(adapter, &manifest.id, &config),
                );
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
