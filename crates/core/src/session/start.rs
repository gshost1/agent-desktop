//! Session creation: a fresh directory and one manifest write, all or nothing.

use super::{
    ArtifactsMode, SessionManifest, SessionTraceMode, new_session_id, now_millis, session_dir,
    tree, validate_session_name, write_manifest,
};
use crate::{AppError, CursorOverlayConfig};
use std::io::ErrorKind;
use std::path::Path;

pub struct StartSessionOptions {
    pub name: Option<String>,
    pub trace: SessionTraceMode,
    pub artifacts: ArtifactsMode,
}

impl Default for StartSessionOptions {
    fn default() -> Self {
        Self {
            name: None,
            trace: SessionTraceMode::On,
            artifacts: ArtifactsMode::Events,
        }
    }
}

pub fn start_session(options: StartSessionOptions) -> Result<SessionManifest, AppError> {
    start_session_with_cursor_overlay(options, CursorOverlayConfig::default())
}

/// Starts a session whose single manifest write already carries
/// `cursor_overlay`. A failure removes the directory it created, so an error
/// always means no session exists, on disk or in `session list`.
pub fn start_session_with_cursor_overlay(
    options: StartSessionOptions,
    cursor_overlay: CursorOverlayConfig,
) -> Result<SessionManifest, AppError> {
    let cursor_overlay = cursor_overlay.validated()?;
    if matches!(options.trace, SessionTraceMode::Off)
        && matches!(options.artifacts, ArtifactsMode::Full)
    {
        return Err(AppError::invalid_input_with_suggestion(
            "Artifacts mode full requires tracing",
            "Remove --no-trace or omit --screenshots.",
        ));
    }
    let id = new_session_id();
    let name = options
        .name
        .map(|name| validate_session_name(&name))
        .transpose()?;
    let dir = session_dir(&id)?;
    let manifest = SessionManifest {
        id,
        name,
        created_at: now_millis(),
        ended_at: None,
        trace: options.trace,
        artifacts: options.artifacts,
        cursor_overlay,
    };
    if let Err(error) = tree::create(&dir).and_then(|()| write_manifest(&manifest)) {
        discard_unstarted(&dir);
        return Err(error);
    }
    Ok(manifest)
}

fn discard_unstarted(dir: &Path) {
    match std::fs::remove_dir_all(dir) {
        Err(error) if error.kind() != ErrorKind::NotFound => tracing::warn!(
            "could not remove unstarted session directory {}: {error}",
            dir.display()
        ),
        _ => {}
    }
}
