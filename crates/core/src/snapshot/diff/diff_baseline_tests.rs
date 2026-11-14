use super::*;
use crate::adapter::minimal_accessibility_node as node;
use crate::refs::RefMap;
use crate::refs::refs_test_support::HomeGuard;
use crate::{SnapshotNotes, SnapshotOwner, WindowInfo};

fn result() -> SnapshotResult {
    let mut root = node("window", "Doc");
    root.children = vec![node("button", "Save")];
    SnapshotResult {
        tree: root,
        refmap: RefMap::new(),
        owner: SnapshotOwner::Window(WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "App".into(),
            pid: crate::ProcessId::new(7),
            process_instance: None,
            bounds: None,
            state: crate::WindowState::default(),
        }),
        snapshot_id: None,
        complete: true,
        nodes_observed: 2,
        notes: SnapshotNotes::default(),
    }
}

fn kind_of(err: AppError) -> (ErrorCode, String) {
    let AppError::Adapter(err) = err else {
        panic!("expected an adapter error, got {err:?}");
    };
    let kind = err.details.as_ref().unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_owned();
    (err.code, kind)
}

#[test]
fn a_saved_baseline_loads_back_from_the_same_session() {
    let _home = HomeGuard::new();
    let store = RefStore::for_session(Some("alpha")).unwrap();
    let id = store.save_new_snapshot(&RefMap::new()).unwrap();
    save_baseline(&store, &id, &result(), &TreeOptions::default()).unwrap();
    let baseline = load_baseline(&store, &id).unwrap();
    assert!(baseline.complete);
    assert_eq!(baseline.owner.window_id.as_deref(), Some("w-1"));
    assert_eq!(baseline.rows.len(), 2);
}

#[test]
fn another_session_never_reads_the_baseline() {
    let _home = HomeGuard::new();
    let alpha = RefStore::for_session(Some("alpha")).unwrap();
    let id = alpha.save_new_snapshot(&RefMap::new()).unwrap();
    save_baseline(&alpha, &id, &result(), &TreeOptions::default()).unwrap();
    let beta = RefStore::for_session(Some("beta")).unwrap();
    let (code, kind) = kind_of(load_baseline(&beta, &id).unwrap_err());
    assert_eq!(code, ErrorCode::SnapshotNotFound);
    assert_eq!(kind, "since_snapshot_not_found");
    let global = RefStore::for_session(None).unwrap();
    assert!(load_baseline(&global, &id).is_err());
}

#[test]
fn an_unknown_id_and_a_snapshot_without_a_baseline_are_told_apart() {
    let _home = HomeGuard::new();
    let store = RefStore::for_session(None).unwrap();
    let (code, kind) = kind_of(load_baseline(&store, "nosuchid").unwrap_err());
    assert_eq!(code, ErrorCode::SnapshotNotFound);
    assert_eq!(kind, "since_snapshot_not_found");

    let id = store.save_new_snapshot(&RefMap::new()).unwrap();
    let (code, kind) = kind_of(load_baseline(&store, &id).unwrap_err());
    assert_eq!(code, ErrorCode::SnapshotNotFound);
    assert_eq!(kind, "diff_baseline_missing");
}

#[test]
fn the_refusal_recovers_through_a_fresh_snapshot() {
    let _home = HomeGuard::new();
    let store = RefStore::for_session(None).unwrap();
    let err = load_baseline(&store, "nosuchid").unwrap_err();
    let payload = crate::response::output::ErrorPayload::from_app_error(&err);
    assert_eq!(payload.code, "SNAPSHOT_NOT_FOUND");
    assert!(payload.suggestion.unwrap().contains("without --since"));
    let recovery = payload.recovery.unwrap();
    assert_eq!(recovery.strategy, "refresh_snapshot_then_retry_original");
    assert!(recovery.requires_fresh_snapshot);
}

#[test]
fn a_baseline_of_another_format_is_treated_as_missing() {
    let _home = HomeGuard::new();
    let store = RefStore::for_session(None).unwrap();
    let id = store.save_new_snapshot(&RefMap::new()).unwrap();
    let mut baseline = DiffBaseline::of(&result(), &TreeOptions::default());
    baseline.format = BASELINE_FORMAT + 1;
    let bytes = serde_json::to_vec(&baseline).unwrap();
    crate::refs::write_private_file(&baseline_path(&store, &id), &bytes).unwrap();
    let (_, kind) = kind_of(load_baseline(&store, &id).unwrap_err());
    assert_eq!(kind, "diff_baseline_missing");
}

#[test]
fn an_invalid_id_is_invalid_args() {
    let _home = HomeGuard::new();
    let store = RefStore::for_session(None).unwrap();
    let AppError::Adapter(err) = load_baseline(&store, "../x").unwrap_err() else {
        panic!("expected an adapter error");
    };
    assert_eq!(err.code, ErrorCode::InvalidArgs);
}
