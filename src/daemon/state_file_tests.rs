use super::DaemonStateFile;

#[test]
fn state_names_this_process_and_the_latest_count() {
    let dir = std::env::temp_dir().join(format!("ad-state-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("k.state");
    let state = DaemonStateFile::create(&path).unwrap();
    state.record(7).unwrap();
    state.record(12).unwrap();

    let read: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    state.remove();
    let gone = !path.exists();
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(read["pid"], std::process::id());
    assert_eq!(read["served"], 12);
    assert!(gone);
}
