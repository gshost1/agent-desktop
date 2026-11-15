use super::BuildIdentity;

#[test]
fn identity_names_the_version_and_changes_when_the_file_changes() {
    let dir = std::env::temp_dir().join(format!("ad-build-identity-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("binary");
    std::fs::write(&file, b"one").unwrap();
    let before = BuildIdentity::from_parts("1.2.3", &std::fs::metadata(&file).unwrap());
    std::fs::remove_file(&file).unwrap();
    std::fs::write(&file, b"second build").unwrap();
    let after = BuildIdentity::from_parts("1.2.3", &std::fs::metadata(&file).unwrap());
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(before.as_str().starts_with("1.2.3:"));
    assert_ne!(before, after);
}

#[test]
fn identity_differs_between_versions_of_the_same_file() {
    let metadata = std::fs::metadata(std::env::current_exe().unwrap()).unwrap();

    assert_ne!(
        BuildIdentity::from_parts("1.0.0", &metadata),
        BuildIdentity::from_parts("1.0.1", &metadata)
    );
}

#[test]
fn current_identity_is_readable_and_stable() {
    assert_eq!(BuildIdentity::current(), BuildIdentity::current());
    assert!(BuildIdentity::current().is_some());
}
