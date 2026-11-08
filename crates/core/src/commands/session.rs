use crate::session::{
    ArtifactsMode, GcOptions, SessionManifest, SessionTraceMode, StartSessionOptions, end_session,
    gc, list_sessions, start_session_with_cursor_overlay,
};
use crate::{AppError, CursorOverlayConfig};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum SessionAction {
    Start {
        name: Option<String>,
        no_trace: bool,
        screenshots: bool,
        cursor_overlay: Option<Box<CursorOverlayConfig>>,
    },
    End {
        id: String,
    },
    List,
    Gc {
        older_than_secs: Option<u64>,
        ended_only: bool,
    },
}

pub fn execute(action: SessionAction) -> Result<Value, AppError> {
    match action {
        SessionAction::Start {
            name,
            no_trace,
            screenshots,
            cursor_overlay,
        } => start(
            name,
            no_trace,
            screenshots,
            cursor_overlay.map(|config| *config),
        )
        .map(|(_, response)| response),
        SessionAction::End { id } => {
            let manifest = end_session(&id)?;
            Ok(json!({
                "session_id": manifest.id,
                "ended_at": manifest.ended_at,
            }))
        }
        SessionAction::List => {
            let sessions: Vec<Value> = list_sessions()?
                .into_iter()
                .map(|manifest| {
                    json!({
                        "session_id": manifest.id,
                        "name": manifest.name,
                        "created_at": manifest.created_at,
                        "ended_at": manifest.ended_at,
                        "trace": manifest.trace,
                        "artifacts": manifest.artifacts,
                    })
                })
                .collect();
            Ok(json!({ "sessions": sessions }))
        }
        SessionAction::Gc {
            older_than_secs,
            ended_only,
        } => {
            let report = gc(GcOptions {
                ended_only,
                older_than: older_than_secs.map(Duration::from_secs),
            })?;
            Ok(json!({ "removed": report.removed }))
        }
    }
}

/// Starts a session, with `cursor_overlay` in the same manifest write when
/// it is given. Every fallible step, including the response's overlay
/// echo, runs before that write, so an error never hides a started session.
/// Returns the manifest beside the response for callers that act on it.
pub fn start(
    name: Option<String>,
    no_trace: bool,
    screenshots: bool,
    cursor_overlay: Option<CursorOverlayConfig>,
) -> Result<(SessionManifest, Value), AppError> {
    let cursor_overlay = cursor_overlay
        .map(CursorOverlayConfig::validated)
        .transpose()?;
    let overlay_echo = cursor_overlay
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;
    let manifest = start_session_with_cursor_overlay(
        StartSessionOptions {
            name,
            trace: if no_trace {
                SessionTraceMode::Off
            } else {
                SessionTraceMode::On
            },
            artifacts: if screenshots {
                ArtifactsMode::Full
            } else {
                ArtifactsMode::Events
            },
        },
        cursor_overlay.unwrap_or_default(),
    )?;
    let mut response = json!({
        "session_id": manifest.id,
        "name": manifest.name,
        "trace": manifest.trace,
        "artifacts": manifest.artifacts,
        "created_at": manifest.created_at,
        "next": activation_export(&manifest.id),
        "activation": activation(&manifest.id),
    });
    if let Some(overlay_echo) = overlay_echo {
        response["cursor_overlay"] = overlay_echo;
    }
    Ok((manifest, response))
}

/// The line a caller pastes into their own shell, so the shell that matters
/// is the one running the command rather than the one this was built for.
/// Written as a runtime branch, not a compile-time one: core is built and
/// tested on every supported platform, and a conditionally-compiled arm is
/// only ever type-checked on the lane that selects it.
pub(super) fn activation_export(session_id: &str) -> String {
    if cfg!(windows) {
        format!("$env:AGENT_DESKTOP_SESSION = '{session_id}'")
    } else {
        format!("export AGENT_DESKTOP_SESSION={session_id}")
    }
}

pub(super) fn activation(session_id: &str) -> Value {
    json!({
        "environment": "AGENT_DESKTOP_SESSION",
        "value": session_id,
    })
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
