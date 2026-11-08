use super::TempDir;

#[test]
fn dropping_the_guard_removes_its_directory_tree() {
    let guard = TempDir::new();
    let root = guard.path().to_path_buf();
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("nested").join("file"), b"x").unwrap();
    drop(guard);
    assert!(!root.exists());
}

#[test]
fn an_adopted_path_is_removed_whether_it_is_a_file_or_missing() {
    let base = TempDir::new();
    let file = base.join("lone-file");
    std::fs::write(&file, b"x").unwrap();
    drop(TempDir::adopt(file.clone()));
    assert!(!file.exists());
    drop(TempDir::adopt(base.join("never-created")));
}
