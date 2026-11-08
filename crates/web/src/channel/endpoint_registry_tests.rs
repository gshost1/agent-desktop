use agent_desktop_core::{Deadline, StateFile};

use super::{read_entries, record_entry};
use crate::channel::registry_entry::RegistryEntry;
use crate::channel::test_dir::TestDir;

fn entry(pid: u32, instance: &str, port: u16) -> RegistryEntry {
    RegistryEntry {
        pid,
        process_instance: instance.into(),
        port,
        websocket_url: format!("ws://127.0.0.1:{port}/devtools/browser/x"),
    }
}

fn deadline() -> Deadline {
    Deadline::after(1_000).unwrap()
}

#[test]
fn a_missing_or_malformed_registry_holds_no_entries() {
    let dir = TestDir::new();
    let file = StateFile::at(dir.path().join("endpoints.json"));
    assert!(read_entries(&file).is_empty());
    file.write(b"not json").unwrap();
    assert!(read_entries(&file).is_empty());
}

#[test]
fn a_relaunch_replaces_the_entry_for_its_process_and_its_port() {
    let dir = TestDir::new();
    let file = StateFile::at(dir.path().join("endpoints.json"));
    record_entry(&file, entry(10, "a", 9222), deadline()).unwrap();
    record_entry(&file, entry(11, "b", 9333), deadline()).unwrap();
    record_entry(&file, entry(10, "a", 9444), deadline()).unwrap();
    record_entry(&file, entry(12, "c", 9333), deadline()).unwrap();
    let entries = read_entries(&file);
    assert_eq!(entries, vec![entry(10, "a", 9444), entry(12, "c", 9333)]);
}

#[test]
fn the_registry_keeps_the_newest_thirty_two() {
    let dir = TestDir::new();
    let file = StateFile::at(dir.path().join("endpoints.json"));
    for index in 0..40u16 {
        record_entry(
            &file,
            entry(u32::from(index), "i", 9000 + index),
            deadline(),
        )
        .unwrap();
    }
    let entries = read_entries(&file);
    assert_eq!(entries.len(), 32);
    assert_eq!(entries[0].port, 9008);
    assert_eq!(entries[31].port, 9039);
}
