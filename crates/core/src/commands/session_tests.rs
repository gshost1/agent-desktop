use super::*;
use crate::refs::refs_test_support::HomeGuard;
use crate::session::{SESSION_MANIFEST_FILE, read_manifest};
use crate::storage::private_file_ops::{PrivateFileOps, with_test_ops_override};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct ManifestWriteFault {
    allowed: usize,
    seen: AtomicUsize,
}

struct Portable;

impl PrivateFileOps for Portable {}

impl PrivateFileOps for ManifestWriteFault {
    fn write_atomic(&self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        if path
            .file_name()
            .is_some_and(|name| name == SESSION_MANIFEST_FILE)
            && self.seen.fetch_add(1, Ordering::SeqCst) >= self.allowed
        {
            return Err(std::io::Error::other("injected manifest write fault"));
        }
        Portable.write_atomic(path, bytes)
    }
}

fn start_with_manifest_fault(allowed: usize) -> Result<Value, AppError> {
    let fault = Rc::new(ManifestWriteFault {
        allowed,
        seen: AtomicUsize::new(0),
    });
    with_test_ops_override(fault, || {
        start(
            None,
            true,
            false,
            CursorOverlayConfig::enabled(None, 6).unwrap(),
        )
    })
}

fn active_session_ids() -> Vec<String> {
    list_sessions()
        .unwrap()
        .into_iter()
        .filter(|manifest| manifest.ended_at.is_none())
        .map(|manifest| manifest.id)
        .collect()
}

#[test]
fn cursor_start_never_hides_an_active_session_behind_an_error() {
    let _home = HomeGuard::new();

    let started = start_with_manifest_fault(1);

    let active = active_session_ids();
    let started = match started {
        Ok(started) => started,
        Err(error) => panic!("start failed with {error} and left active sessions {active:?}"),
    };
    let id = started["session_id"].as_str().unwrap();
    assert_eq!(active, [id]);
    assert_eq!(started["cursor_overlay"]["enabled"], true);
    let manifest = read_manifest(id).unwrap().unwrap();
    assert!(!manifest.cursor_overlay.is_disabled());
    assert!(end_session(id).unwrap().ended_at.is_some());
    assert!(active_session_ids().is_empty());
}

#[test]
fn cursor_start_that_cannot_persist_leaves_no_session() {
    let _home = HomeGuard::new();

    let error = start_with_manifest_fault(0).expect_err("the only manifest write failed");

    assert!(error.to_string().contains("injected manifest write fault"));
    assert!(list_sessions().unwrap().is_empty());
}

#[test]
fn plain_start_carries_no_cursor_overlay() {
    let _home = HomeGuard::new();

    let started = start(None, true, false, CursorOverlayConfig::default()).unwrap();

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
