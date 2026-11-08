use super::*;
use crate::refs::refs_test_support::HomeGuard;
use crate::session::{SESSION_MANIFEST_FILE, read_manifest};
use crate::storage::private_file_ops::{PrivateFileOps, with_test_ops_override};
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

thread_local! {
    static MANIFEST_WRITES: Cell<usize> = const { Cell::new(0) };
}

struct ManifestWrites {
    fail: bool,
}

struct Portable;

impl PrivateFileOps for Portable {}

impl PrivateFileOps for ManifestWrites {
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        if path
            .file_name()
            .is_some_and(|name| name == SESSION_MANIFEST_FILE)
        {
            MANIFEST_WRITES.with(|writes| writes.set(writes.get() + 1));
            if self.fail {
                return Err(std::io::Error::other("injected manifest write fault"));
            }
        }
        Portable.write_atomic(path, bytes)
    }
}

fn start_counting_manifest_writes(
    cursor_overlay: Option<CursorOverlayConfig>,
    fail: bool,
) -> (Result<Value, AppError>, usize) {
    MANIFEST_WRITES.with(|writes| writes.set(0));
    let result = with_test_ops_override(Rc::new(ManifestWrites { fail }), || {
        execute(SessionAction::Start {
            name: None,
            no_trace: true,
            screenshots: false,
            cursor_overlay: cursor_overlay.map(Box::new),
        })
    });
    (result, MANIFEST_WRITES.with(Cell::get))
}

#[test]
fn cursor_start_persists_the_overlay_in_its_only_manifest_write() {
    let _home = HomeGuard::new();
    let cursor = CursorOverlayConfig::enabled(None, 6).unwrap();

    let (started, writes) = start_counting_manifest_writes(Some(cursor), false);

    assert_eq!(
        writes, 1,
        "a second manifest write can fail after the session is active"
    );
    let started = started.expect("cursor start");
    assert_eq!(started["cursor_overlay"]["enabled"], true);
    let id = started["session_id"].as_str().unwrap();
    assert!(
        read_manifest(id)
            .unwrap()
            .unwrap()
            .cursor_overlay
            .is_enabled()
    );
    assert!(end_session(id).unwrap().ended_at.is_some());
}

#[test]
fn failed_start_leaves_no_session_and_no_directory() {
    let _home = HomeGuard::new();
    let cursor = CursorOverlayConfig::enabled(None, 6).unwrap();

    let (started, writes) = start_counting_manifest_writes(Some(cursor), true);

    assert_eq!(writes, 1);
    let error = started.expect_err("the manifest write failed");
    assert!(error.to_string().contains("injected manifest write fault"));
    assert!(list_sessions().unwrap().is_empty());
    let leftovers: Vec<_> = std::fs::read_dir(
        crate::session::agent_desktop_dir()
            .unwrap()
            .join("sessions"),
    )
    .map(|entries| entries.flatten().map(|entry| entry.file_name()).collect())
    .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "unstarted session left on disk: {leftovers:?}"
    );
}

#[test]
fn plain_start_echoes_no_cursor_overlay() {
    let _home = HomeGuard::new();

    let (started, writes) = start_counting_manifest_writes(None, false);

    assert_eq!(writes, 1);
    let started = started.unwrap();
    assert!(started.get("cursor_overlay").is_none());
    let id = started["session_id"].as_str().unwrap();
    assert!(
        read_manifest(id)
            .unwrap()
            .unwrap()
            .cursor_overlay
            .is_disabled()
    );
}
