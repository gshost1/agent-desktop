use super::StateFile;
use crate::refs::refs_test_support::HomeGuard;

#[test]
fn a_missing_file_reads_as_none_and_a_write_round_trips() {
    let _home = HomeGuard::new();
    let file = StateFile::under_state_root(&["web", "endpoints.json"]).unwrap();
    assert!(file.read(1024).unwrap().is_none());
    file.write(b"{\"entries\":[]}").unwrap();
    assert_eq!(file.read(1024).unwrap().unwrap(), b"{\"entries\":[]}");
    assert!(file.path().ends_with("web/endpoints.json"));
}

#[test]
fn an_oversized_file_is_refused() {
    let _home = HomeGuard::new();
    let file = StateFile::under_state_root(&["big.json"]).unwrap();
    file.write(&[b'x'; 64]).unwrap();
    assert!(file.read(8).is_err());
}

#[test]
fn path_segments_must_be_plain_names() {
    for segments in [&[][..], &[".."][..], &["a/b"][..], &[""][..]] {
        assert!(StateFile::under_state_root(segments).is_err());
    }
}

#[test]
fn the_lock_is_exclusive_until_dropped() {
    let _home = HomeGuard::new();
    let file = StateFile::under_state_root(&["web", "endpoints.json"]).unwrap();
    let held = file
        .lock(crate::Deadline::after(500).unwrap(), "test")
        .unwrap();
    let blocked = file.lock(crate::Deadline::after(50).unwrap(), "test");
    assert!(blocked.is_err());
    drop(held);
    file.lock(crate::Deadline::after(500).unwrap(), "test")
        .unwrap();
}
