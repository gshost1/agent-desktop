use super::*;
use crate::refs::refs_test_support::HomeGuard;
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
