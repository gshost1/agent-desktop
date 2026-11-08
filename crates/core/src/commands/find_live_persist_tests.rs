use super::*;
use crate::refs::refs_test_support::HomeGuard;

#[test]
fn a_built_find_refmap_is_persisted_after_the_inherited_deadline_expires() {
    let _guard = HomeGuard::new();
    let inherited = crate::Deadline::after(0).unwrap();
    let deadline_scope = crate::wait::deadline::enter_scope(Some(inherited));

    let snapshot_id = persist_refmap(&CommandContext::default(), &RefMap::new())
        .expect("a complete find result must not be discarded at save time");
    drop(deadline_scope);

    let store = RefStore::new().unwrap();
    assert_eq!(
        store.latest_snapshot_id().unwrap(),
        Some(snapshot_id.clone())
    );
    assert!(store.load_snapshot(&snapshot_id).is_ok());
}
