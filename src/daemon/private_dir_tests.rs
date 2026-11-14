use super::{check_private_dir, ensure_private_dir};
use std::os::unix::fs::PermissionsExt;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ad-private-dir-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn created_directory_is_private() {
    let root = scratch("created");
    let dir = root.join("a").join("daemon");

    ensure_private_dir(&dir).unwrap();
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(mode & 0o777, 0o700);
}

#[test]
fn directory_open_to_others_is_refused() {
    let root = scratch("open");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();

    let refused = check_private_dir(&root).is_err();
    let ensured = ensure_private_dir(&root).is_err();
    std::fs::remove_dir_all(&root).unwrap();

    assert!(refused);
    assert!(ensured);
}

#[test]
fn symlink_to_a_private_directory_is_refused() {
    let root = scratch("link");
    let real = root.join("real");
    ensure_private_dir(&real).unwrap();
    let link = root.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let refused = check_private_dir(&link).is_err();
    std::fs::remove_dir_all(&root).unwrap();

    assert!(refused);
}
