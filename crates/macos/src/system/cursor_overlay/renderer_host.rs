//! Whether this process may start the overlay renderer.
//!
//! The renderer is this process's own executable started again with the
//! child marker set. That is only safe when the executable's `main` checks
//! the marker first, which the CLI does by calling
//! `run_cursor_overlay_child` before anything else. An FFI host such as
//! python or node, or a test binary, never makes that call, and starting it
//! again would run the host program instead of a renderer. The process
//! claims the role by making the call, so a renamed or wrapped CLI binary
//! still draws, and its file name is never the test.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_desktop_core::AdapterError;

static RENDERER_HOST: AtomicBool = AtomicBool::new(false);

/// Runs the renderer when this process was started as one, and otherwise
/// records that this executable answers the child marker.
pub(crate) fn entry_or_claim() -> Option<Result<(), AdapterError>> {
    claim_unless_child(super::child::entry_from_env(), &RENDERER_HOST)
}

fn claim_unless_child(
    entry: Option<Result<(), AdapterError>>,
    host: &AtomicBool,
) -> Option<Result<(), AdapterError>> {
    if entry.is_none() {
        host.store(true, Ordering::Release);
    }
    entry
}

/// The executable to start as the renderer. A process that never claimed
/// the role refuses with `PLATFORM_NOT_SUPPORTED`: the CLI's dispatch
/// reports it as `rendered: false`, and action presentation is skipped.
pub(crate) fn executable() -> Result<PathBuf, AdapterError> {
    renderer_executable(
        RENDERER_HOST.load(Ordering::Acquire),
        std::env::current_exe(),
    )
}

fn renderer_executable(
    hosted: bool,
    current: std::io::Result<PathBuf>,
) -> Result<PathBuf, AdapterError> {
    if !hosted {
        return Err(
            AdapterError::not_supported("update_cursor_overlay").with_platform_detail(
                "the overlay renderer is started only by the agent-desktop CLI; \
                 this process (an FFI host or a test binary) does not run its renderer entry",
            ),
        );
    }
    current.map_err(|error| {
        AdapterError::internal("Could not locate the cursor overlay executable")
            .with_platform_detail(error.to_string())
    })
}

#[cfg(test)]
#[path = "renderer_host_tests.rs"]
mod tests;
