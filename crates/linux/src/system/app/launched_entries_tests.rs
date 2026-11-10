use super::LaunchedEntries;
use std::path::Path;

fn store(name: &str) -> LaunchedEntries {
    let dir = std::env::temp_dir().join(format!("ad-launched-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    LaunchedEntries { dir }
}

fn this_instance() -> (u32, String) {
    let pid = std::process::id();
    let facts = crate::system::process::process_facts::read(pid).expect("own facts");
    (pid, facts.instance)
}

#[test]
fn a_recorded_entry_answers_for_its_pid_instance_and_executable_only() {
    let store = store("recall");
    let (pid, instance) = this_instance();
    let browser = Path::new("/opt/browser/chrome");
    store.remember((pid, &instance), browser, "com.example.Browser");
    assert_eq!(
        store.recall(pid, &instance, browser).as_deref(),
        Some("com.example.Browser")
    );
    assert_eq!(store.recall(pid + 1, &instance, browser), None);
    assert_eq!(store.recall(pid, "boot:other", browser), None);
    assert_eq!(
        store.recall(pid, &instance, Path::new("/usr/bin/other")),
        None
    );
    let _ = std::fs::remove_dir_all(&store.dir);
}

#[test]
fn records_of_processes_that_are_gone_are_dropped_and_live_ones_kept() {
    let store = store("prune");
    let (pid, instance) = this_instance();
    let mut child = std::process::Command::new("/bin/true")
        .spawn()
        .expect("a child");
    let gone = child.id();
    let gone_instance = crate::system::process::process_facts::read(gone)
        .map(|facts| facts.instance)
        .unwrap_or_else(|| "boot:gone".to_owned());
    child.wait().expect("the child exits");
    store.remember((gone, &gone_instance), Path::new("/bin/true"), "gone");
    store.remember((pid, &instance), Path::new("/bin/app"), "live");
    assert_eq!(
        store.recall(gone, &gone_instance, Path::new("/bin/true")),
        None
    );
    assert_eq!(
        store
            .recall(pid, &instance, Path::new("/bin/app"))
            .as_deref(),
        Some("live")
    );
    let _ = std::fs::remove_dir_all(&store.dir);
}

#[test]
fn the_store_keeps_the_newest_records_when_it_is_full() {
    let store = store("full");
    let (pid, instance) = this_instance();
    let mut last = std::path::PathBuf::new();
    for index in 0..(super::KEPT + 4) {
        last = store.path(pid, &format!("{instance}-{index}"));
        let text = format!("{pid}\n/bin/app\napp");
        super::write_private_atomically(&store.dir, &last, text.as_bytes()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    store.prune(&last, |_| false);
    let kept = std::fs::read_dir(&store.dir).unwrap().count();
    assert_eq!(kept, super::KEPT);
    let newest = format!("{instance}-{}", super::KEPT + 3);
    let oldest = format!("{instance}-0");
    assert!(store.recall(pid, &newest, Path::new("/bin/app")).is_some());
    assert!(store.recall(pid, &oldest, Path::new("/bin/app")).is_none());
    let _ = std::fs::remove_dir_all(&store.dir);
}
