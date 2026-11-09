use super::Scratch;

#[test]
fn dropping_the_guard_removes_a_directory_tree() {
    let root = std::env::temp_dir().join(format!("ad-scratch-guard-{}", std::process::id()));
    let guard = Scratch::adopt(root.clone());
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("nested").join("file"), b"x").unwrap();
    drop(guard);
    assert!(!root.exists());
}

#[test]
fn dropping_the_guard_removes_a_plain_file() {
    let path = std::env::temp_dir().join(format!("ad-scratch-file-{}", std::process::id()));
    let guard = Scratch::adopt(path.clone());
    std::fs::write(&path, b"x").unwrap();
    drop(guard);
    assert!(!path.exists());
}
