use super::*;
use crate::refs::refs_test_support::{
    HandshakeChild, SUBPROCESS_HANG_LIMIT, signal_ready_and_wait_for_release,
};
use crate::{RefCapabilities, RefEntryIdentity, RefGeometry, RefProcess, RefScope, RefSource};

fn entry(name: &str, root_ref: Option<&str>) -> crate::RefEntry {
    crate::RefEntry {
        process: RefProcess {
            pid: crate::ProcessId::new(7),
            process_instance: Some("instance-1".into()),
        },
        identity: RefEntryIdentity {
            retained_object: None,
            role: "button".into(),
            name: Some(name.into()),
            value: None,
            description: None,
            native_id: None,
            capture: crate::CaptureVerdict::Unique,
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: RefCapabilities {
            states: Vec::new(),
            available_actions: vec![crate::actions::capability::CLICK.into()],
        },
        source: RefSource {
            source_app: Some("Fixture".into()),
            source_window_id: Some("w-1".into()),
            source_window_title: Some("Fixture".into()),
            source_window_bounds_hash: None,
            source_surface: crate::SnapshotSurface::Window,
            renderer_accessibility_forced: false,
        },
        scope: RefScope {
            root_ref: root_ref.map(str::to_string),
            path_is_absolute: root_ref.is_some(),
            path: smallvec::SmallVec::new(),
        },
    }
}

#[test]
fn subprocess_transaction_writer() {
    let Some(snapshot_id) = std::env::var_os("AGENT_DESKTOP_TX_SNAPSHOT") else {
        return;
    };
    let root_ref = std::env::var("AGENT_DESKTOP_TX_ROOT_REF").unwrap();
    let label = std::env::var("AGENT_DESKTOP_TX_LABEL").unwrap();
    let store = RefStore::new().unwrap();
    let snapshot_id = snapshot_id.to_string_lossy();
    let expected = store
        .load_snapshot(&snapshot_id)
        .unwrap()
        .get(&root_ref)
        .unwrap()
        .clone();
    signal_ready_and_wait_for_release();
    let hang_at = std::time::Instant::now() + SUBPROCESS_HANG_LIMIT;
    loop {
        let attempt =
            store.update_existing_snapshot(&snapshot_id, &root_ref, &expected, |current| {
                current.try_allocate(entry(&label, Some(&root_ref)))?;
                Ok(())
            });
        match attempt {
            Ok(_) => return,
            Err(error) if is_lock_timeout(&error) && std::time::Instant::now() < hang_at => {}
            Err(error) => panic!("transaction failed: {error:?}"),
        }
    }
}

fn is_lock_timeout(error: &AppError) -> bool {
    matches!(error, AppError::Adapter(adapter) if adapter
        .details
        .as_ref()
        .is_some_and(|details| details["kind"] == "lock_timeout"))
}

#[test]
fn two_process_transactions_preserve_disjoint_drill_updates() {
    let home = std::env::temp_dir().join(format!(
        "agent-desktop-refstore-two-process-{}",
        crate::refs::new_snapshot_id()
    ));
    std::fs::create_dir_all(&home).unwrap();
    let previous = crate::refs::set_home_override(Some(home.clone()));
    let store = RefStore::new().unwrap();
    let mut map = RefMap::new();
    map.try_allocate(entry("Root A", None)).unwrap();
    map.try_allocate(entry("Root B", None)).unwrap();
    let snapshot_id = store.save_new_snapshot(&map).unwrap();
    let mut children = [("@e1", "Child A"), ("@e2", "Child B")].map(|(root_ref, label)| {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("refs::refs_store::transaction_tests::subprocess_transaction_writer")
            .arg("--nocapture")
            .env("HOME", &home)
            .env("AGENT_DESKTOP_TX_SNAPSHOT", &snapshot_id)
            .env("AGENT_DESKTOP_TX_ROOT_REF", root_ref)
            .env("AGENT_DESKTOP_TX_LABEL", label);
        HandshakeChild::spawn(command)
    });
    for child in &mut children {
        child.wait_ready();
    }
    for child in &mut children {
        child.release();
    }
    for child in &mut children {
        assert!(child.wait_exit().success());
    }
    let updated = store.load_snapshot(&snapshot_id).unwrap();
    assert_eq!(updated.len(), 4);
    assert!(updated.get("@e1").is_some());
    assert!(updated.get("@e2").is_some());
    let names = ["@e3", "@e4"]
        .into_iter()
        .map(|id| updated.get(id).unwrap().identity.name.as_deref().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        names,
        std::collections::HashSet::from(["Child A", "Child B"])
    );
    crate::refs::set_home_override(previous);
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn subprocess_crash_before_snapshot_rename() {
    let Some(snapshot_id) = std::env::var_os("AGENT_DESKTOP_CRASH_SNAPSHOT") else {
        return;
    };
    let store = RefStore::new().unwrap();
    let snapshot_id = snapshot_id.to_string_lossy();
    let expected = store
        .load_snapshot(&snapshot_id)
        .unwrap()
        .get("@e1")
        .unwrap()
        .clone();
    let _ = store.update_existing_snapshot(&snapshot_id, "@e1", &expected, |current| {
        current.try_allocate(entry("Never committed", Some("@e1")))?;
        Ok(())
    });
    panic!("crash failpoint did not abort");
}

#[test]
fn crash_before_rename_preserves_the_previous_snapshot() {
    let home = std::env::temp_dir().join(format!(
        "agent-desktop-refstore-crash-{}",
        crate::refs::new_snapshot_id()
    ));
    std::fs::create_dir_all(&home).unwrap();
    let previous = crate::refs::set_home_override(Some(home.clone()));
    let store = RefStore::new().unwrap();
    let mut map = RefMap::new();
    map.try_allocate(entry("Original", None)).unwrap();
    let snapshot_id = store.save_new_snapshot(&map).unwrap();
    let snapshot_path = store.snapshot_path(&snapshot_id);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("refs::refs_store::transaction_tests::subprocess_crash_before_snapshot_rename")
        .arg("--nocapture")
        .env("HOME", &home)
        .env("AGENT_DESKTOP_CRASH_SNAPSHOT", &snapshot_id)
        .env("AGENT_DESKTOP_TEST_CRASH_BEFORE_RENAME", &snapshot_path)
        .status()
        .unwrap();

    assert!(!status.success());
    let persisted = store.load_snapshot(&snapshot_id).unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(
        persisted.get("@e1").unwrap().identity.name.as_deref(),
        Some("Original")
    );
    crate::refs::set_home_override(previous);
    std::fs::remove_dir_all(home).unwrap();
}
