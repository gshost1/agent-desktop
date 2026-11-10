use super::LaunchedEntries;
use std::path::Path;

fn store(name: &str) -> LaunchedEntries {
    let dir = std::env::temp_dir().join(format!("ad-launched-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    LaunchedEntries { dir }
}

#[test]
fn a_recorded_entry_answers_for_its_instance_and_executable_only() {
    let store = store("recall");
    let chrome = Path::new("/opt/browser/chrome");
    store.remember("boot:41", chrome, "com.example.Browser");
    assert_eq!(
        store.recall("boot:41", chrome).as_deref(),
        Some("com.example.Browser")
    );
    assert_eq!(store.recall("boot:42", chrome), None);
    assert_eq!(store.recall("boot:41", Path::new("/usr/bin/other")), None);
    let _ = std::fs::remove_dir_all(&store.dir);
}

#[test]
fn the_store_keeps_only_the_newest_records() {
    let store = store("prune");
    for index in 0..(super::KEPT + 4) {
        store.remember(&format!("boot:{index}"), Path::new("/bin/app"), "app");
    }
    let kept = std::fs::read_dir(&store.dir).unwrap().count();
    assert_eq!(kept, super::KEPT);
    let _ = std::fs::remove_dir_all(&store.dir);
}
