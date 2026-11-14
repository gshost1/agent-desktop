use super::*;
use crate::refs::refs_test_support::HomeGuard;
use crate::refs::write_private_cache_file;
use crate::storage::write_durability::syncs_on_this_thread;

fn refmap(refs: usize) -> RefMap {
    let mut map = RefMap::new();
    for index in 0..refs {
        map.allocate(crate::refs::tests::entry(
            "button",
            Some(&format!("b{index}")),
        ));
    }
    map
}

fn torn_refmaps(intact: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("empty", Vec::new()),
        ("zero-filled", vec![0; intact.len()]),
        ("cut in half", intact[..intact.len() / 2].to_vec()),
        ("missing its last byte", intact[..intact.len() - 1].to_vec()),
        ("garbage", b"\x7fELF not json".to_vec()),
        ("not a ref map", b"[1, 2, 3]".to_vec()),
    ]
}

fn assert_snapshot_not_found(result: Result<impl std::fmt::Debug, AppError>, form: &str) {
    match result {
        Ok(value) => panic!("a {form} ref map read as {value:?}"),
        Err(err) => assert_eq!(err.code(), "SNAPSHOT_NOT_FOUND", "{form}: {err}"),
    }
}

#[test]
fn a_snapshot_save_issues_no_disk_flush() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let map = refmap(3);
    let before = syncs_on_this_thread();
    let first = store.save_new_snapshot(&map).unwrap();
    let second = store.save_new_snapshot(&map).unwrap();
    assert_eq!(
        syncs_on_this_thread() - before,
        0,
        "a refmap or latest-pointer write flushed to disk; that costs about 18 ms per snapshot"
    );
    assert_eq!(store.load_snapshot(&first).unwrap().len(), 3);
    assert_eq!(
        store.latest_snapshot_id().unwrap().as_deref(),
        Some(second.as_str())
    );
}

#[test]
fn a_torn_ref_map_reads_as_a_missing_snapshot_for_every_reader() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let id = store.save_new_snapshot(&refmap(3)).unwrap();
    let path = store.snapshot_path(&id);
    let intact = std::fs::read(&path).unwrap();
    for (form, bytes) in torn_refmaps(&intact) {
        write_private_cache_file(&path, &bytes).unwrap();
        assert_snapshot_not_found(store.load_snapshot(&id), form);
        assert_snapshot_not_found(store.load(Some(&id)), form);
        assert_snapshot_not_found(store.load_latest(), form);
        assert_snapshot_not_found(store.load_ref(&format!("@{id}:e1")), form);
    }
}

#[test]
fn no_prefix_of_a_ref_map_reads_as_a_snapshot() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let id = store.save_new_snapshot(&refmap(2)).unwrap();
    let path = store.snapshot_path(&id);
    let intact = std::fs::read(&path).unwrap();
    for length in 0..intact.len() {
        write_private_cache_file(&path, &intact[..length]).unwrap();
        assert_snapshot_not_found(store.load_snapshot(&id), &format!("{length}-byte"));
    }
    write_private_cache_file(&path, &intact).unwrap();
    assert_eq!(store.load_snapshot(&id).unwrap().len(), 2);
}

#[test]
fn a_torn_latest_pointer_reads_as_no_latest_snapshot() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let id = store.save_new_snapshot(&refmap(1)).unwrap();
    let cut_to_three = id[..3].as_bytes().to_vec();
    let torn: [(&str, Vec<u8>); 6] = [
        ("empty", Vec::new()),
        ("zero-filled", vec![0; id.len()]),
        ("cut to two bytes", id[..2].as_bytes().to_vec()),
        ("cut to three bytes", cut_to_three),
        ("not UTF-8", vec![0xff, 0xfe, 0xfd]),
        ("garbage", b"s1 s2/../x".to_vec()),
    ];
    for (form, bytes) in torn {
        write_private_cache_file(&store.latest_path(), &bytes).unwrap();
        let latest = store.latest_snapshot_id().unwrap();
        assert!(
            latest.as_deref().is_none_or(|read| read != id),
            "{form}: {latest:?}"
        );
        assert_snapshot_not_found(store.load_latest(), form);
        assert_snapshot_not_found(store.load(None), form);
    }
}

#[test]
fn the_next_snapshot_recovers_from_torn_state() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let torn = store.save_new_snapshot(&refmap(1)).unwrap();
    write_private_cache_file(&store.snapshot_path(&torn), b"").unwrap();
    write_private_cache_file(&store.latest_path(), &[0; 8]).unwrap();
    let fresh = store.save_new_snapshot(&refmap(2)).unwrap();
    assert_eq!(store.load_latest().unwrap().len(), 2);
    assert_eq!(
        store.latest_snapshot_id().unwrap().as_deref(),
        Some(fresh.as_str())
    );
    assert_snapshot_not_found(store.load_snapshot(&torn), "empty");
}

#[test]
fn a_torn_ref_map_names_the_unreadable_file_in_its_detail() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let id = store.save_new_snapshot(&refmap(1)).unwrap();
    write_private_cache_file(&store.snapshot_path(&id), b"{\"next").unwrap();
    let err = store.load_snapshot(&id).unwrap_err();
    let AppError::Adapter(adapter) = err else {
        panic!("expected an adapter error, got {err:?}");
    };
    assert!(
        adapter
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.starts_with("refmap.json is unreadable")),
        "{adapter:?}"
    );
}
