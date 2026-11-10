use super::syncs_on_this_thread;
use crate::refs::refs_test_support::TempDir;
use crate::storage::private_file::{write_atomic_cache_portable, write_atomic_portable};

fn private_directory(root: &TempDir) -> std::path::PathBuf {
    let directory = root.join("private");
    crate::storage::private_file_parent::ensure_private_portable(&directory).unwrap();
    directory
}

#[test]
fn a_durable_write_flushes_the_file_and_its_directory() {
    let root = TempDir::new();
    let directory = private_directory(&root);
    let before = syncs_on_this_thread();
    write_atomic_portable(&directory.join("state.json"), b"durable").unwrap();
    assert_eq!(syncs_on_this_thread() - before, 2);
}

#[test]
fn a_cache_write_replaces_atomically_without_flushing() {
    let root = TempDir::new();
    let directory = private_directory(&root);
    let path = directory.join("refmap.json");
    let before = syncs_on_this_thread();
    write_atomic_cache_portable(&path, b"first").unwrap();
    write_atomic_cache_portable(&path, b"second").unwrap();
    assert_eq!(syncs_on_this_thread() - before, 0);
    assert_eq!(std::fs::read(&path).unwrap(), b"second");
    let leftovers: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name != "refmap.json")
        .collect();
    assert!(
        leftovers.is_empty(),
        "temporaries left behind: {leftovers:?}"
    );
}
