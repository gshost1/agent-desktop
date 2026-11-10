use super::*;
use crate::{
    adapter::SnapshotSurface,
    refs::refs_test_support::HomeGuard,
    refs::{RefEntry, RefMap},
};

fn entry(pid: u32) -> RefEntry {
    RefEntry {
        process: crate::RefProcess {
            pid: ProcessId::new(pid),
            process_instance: Some("test-instance".into()),
        },
        identity: crate::RefEntryIdentity {
            retained_object: None,
            role: "button".into(),
            name: Some("Send".into()),
            value: None,
            description: None,
            native_id: None,
            capture: crate::CaptureVerdict::Unique,
        },
        geometry: crate::RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: crate::RefCapabilities {
            states: vec![],
            available_actions: vec![crate::actions::capability::CLICK.into()],
        },
        source: crate::RefSource {
            source_app: Some("TestApp".into()),
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
            renderer_accessibility_forced: false,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::SmallVec::new(),
        },
    }
}

fn saved_snapshot(session_id: Option<&str>) -> String {
    let mut map = RefMap::new();
    map.allocate(entry(41));
    map.allocate(entry(42));
    RefStore::for_session(session_id)
        .unwrap()
        .save_new_snapshot(&map)
        .unwrap()
}

#[test]
fn a_saved_ref_names_the_process_it_was_captured_from() {
    let _home = HomeGuard::new();
    let snapshot_id = saved_snapshot(None);
    assert_eq!(
        saved_ref_process(&format!("@{snapshot_id}:e2"), None),
        Some(ProcessId::new(42))
    );
}

#[test]
fn a_session_ref_is_read_only_from_its_own_session() {
    let _home = HomeGuard::new();
    let snapshot_id = saved_snapshot(Some("agent-a"));
    let ref_id = format!("@{snapshot_id}:e1");
    assert_eq!(
        saved_ref_process(&ref_id, Some("agent-a")),
        Some(ProcessId::new(41))
    );
    assert_eq!(saved_ref_process(&ref_id, None), None);
    assert_eq!(saved_ref_process(&ref_id, Some("agent-b")), None);
}

#[test]
fn an_unreadable_ref_names_no_process() {
    let _home = HomeGuard::new();
    let snapshot_id = saved_snapshot(None);
    assert_eq!(saved_ref_process(&format!("@{snapshot_id}:e3"), None), None);
    assert_eq!(saved_ref_process("@e1", None), None);
    assert_eq!(saved_ref_process("@sabcdefgh:e1", None), None);
    assert_eq!(
        saved_ref_process(&format!("@{snapshot_id}:e1"), Some("../x")),
        None
    );
}
