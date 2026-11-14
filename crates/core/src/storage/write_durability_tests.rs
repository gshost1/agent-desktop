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

#[test]
fn a_private_state_write_still_flushes_twice() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let root = crate::storage::state_root::resolve_configured_state_root().unwrap();
    let before = syncs_on_this_thread();
    crate::refs::write_private_file(&root.join("state.json"), b"durable").unwrap();
    assert_eq!(syncs_on_this_thread() - before, 2);
}

#[test]
fn a_session_manifest_write_still_flushes_twice() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let manifest = crate::session::SessionManifest {
        id: "durable-1".into(),
        name: None,
        created_at: 1,
        ended_at: None,
        trace: crate::session::SessionTraceMode::Off,
        artifacts: crate::session::ArtifactsMode::Events,
        cursor_overlay: crate::CursorOverlayConfig::default(),
    };
    let before = syncs_on_this_thread();
    crate::session::write_manifest(&manifest).unwrap();
    assert_eq!(syncs_on_this_thread() - before, 2);
}

#[test]
fn a_state_file_write_still_flushes_twice() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let file = crate::storage::state_file::StateFile::under_state_root(&["durable", "state.json"])
        .unwrap();
    let before = syncs_on_this_thread();
    file.write(b"{}").unwrap();
    assert_eq!(syncs_on_this_thread() - before, 2);
}

fn rust_sources(directory: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !path.to_string_lossy().ends_with("_tests.rs")
        {
            found.push(path);
        }
    }
}

#[test]
fn only_the_ref_store_writes_without_a_flush() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    for tree in ["crates", "src"] {
        rust_sources(&workspace.join(tree), &mut sources);
    }
    let mut cache_writers: Vec<(String, usize)> = sources
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            let calls = text.matches("write_private_cache_file(").count()
                + text.matches("write_atomic_cache(").count()
                + text.matches("write_atomic_cache_portable(").count()
                + text.matches("WriteDurability::CacheOnly").count();
            let relative = path.strip_prefix(&workspace).unwrap();
            (calls > 0).then(|| (relative.to_string_lossy().replace('\\', "/"), calls))
        })
        .collect();
    cache_writers.sort();
    assert_eq!(
        cache_writers,
        vec![
            ("crates/core/src/refs/mod.rs".to_string(), 2),
            ("crates/core/src/refs/refs_store.rs".to_string(), 2),
            ("crates/core/src/storage/private_file.rs".to_string(), 4),
            ("crates/core/src/storage/private_file_ops.rs".to_string(), 3),
        ],
        "a new caller writes without a flush; only refmap.json and latest_snapshot_id may"
    );
}
