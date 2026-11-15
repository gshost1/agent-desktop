use super::*;
#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::{Mutex, MutexGuard};

static CURRENT_DIRECTORY_LOCK: Mutex<()> = Mutex::new(());

struct CurrentDirectoryGuard {
    previous: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl CurrentDirectoryGuard {
    fn enter(target: &Path) -> Self {
        let lock = CURRENT_DIRECTORY_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(target).unwrap();
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for CurrentDirectoryGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

fn directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-private-{label}-{}",
        crate::refs::new_snapshot_id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn private_open_sets_nonblocking_and_close_on_exec() {
    let directory = directory("flags");
    let path = directory.join("lock");
    let file = open_private_lock(&path, true).unwrap();

    let descriptor_flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
    let status_flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    assert_ne!(descriptor_flags & libc::FD_CLOEXEC, 0);
    assert_ne!(status_flags & libc::O_NONBLOCK, 0);
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn private_open_rejects_symlink_fifo_device_and_hardlink() {
    let directory = directory("special");
    let target = directory.join("target");
    let target_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&target)
        .unwrap();
    drop(target_file);
    let symlink = directory.join("symlink");
    std::os::unix::fs::symlink(&target, &symlink).unwrap();
    assert!(open_private_lock(&symlink, false).is_err());

    let fifo = directory.join("fifo");
    let fifo_path = CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) }, 0);
    let started = std::time::Instant::now();
    assert!(open_private_lock(&fifo, false).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));

    let hardlink = directory.join("hardlink");
    std::fs::hard_link(&target, &hardlink).unwrap();
    assert!(open_private_lock(&hardlink, false).is_err());
    assert!(open_private_lock(Path::new("/dev/null"), false).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn private_read_enforces_the_bound_before_allocating() {
    let directory = directory("bound");
    let path = directory.join("data");
    write_atomic(&path, b"12345").unwrap();

    let error = read_private_bounded(&path, 4).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn private_writes_and_locks_reject_group_accessible_parent() {
    let directory = directory("hostile-parent");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o770)).unwrap();
    let path = directory.join("data");

    assert_eq!(
        write_atomic(&path, b"private").unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        open_private_lock(&path, true).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn private_write_rejects_an_intermediate_directory_symlink() {
    let directory = directory("intermediate-symlink");
    let outside = directory.join("outside");
    let nested = outside.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&nested, std::fs::Permissions::from_mode(0o700)).unwrap();
    let link = directory.join("redirect");
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let error = write_atomic(&link.join("nested/artifact"), b"private").unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(!nested.join("artifact").exists());
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn user_write_overwrites_an_existing_group_readable_file() {
    let directory = directory("user-overwrite");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = directory.join("out.png");
    std::fs::write(&path, b"old").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    write_user_atomic(&path, b"new").unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn user_write_refuses_symlink_and_directory_destinations() {
    let directory = directory("user-refuse");
    let target = directory.join("target");
    std::fs::write(&target, b"kept").unwrap();
    let symlink = directory.join("symlink");
    std::os::unix::fs::symlink(&target, &symlink).unwrap();
    let subdirectory = directory.join("subdirectory");
    std::fs::create_dir(&subdirectory).unwrap();

    assert_eq!(
        write_user_atomic(&symlink, b"new").unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(
        write_user_atomic(&subdirectory, b"new").unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"kept");

    let error = crate::refs::write_user_file(&symlink, b"new").unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(
        error
            .suggestion()
            .unwrap()
            .contains(&symlink.display().to_string())
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn private_write_still_rejects_a_group_readable_destination() {
    let directory = directory("private-loose-destination");
    let path = directory.join("data");
    std::fs::write(&path, b"old").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let error = write_atomic(&path, b"new").unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn user_write_allows_the_system_temporary_directory() {
    let path = Path::new("/tmp").join(format!(
        "agent-desktop-user-output-{}",
        crate::refs::new_snapshot_id()
    ));

    write_user_atomic(&path, b"private").unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"private");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn atomic_write_lands_content_and_replaces_the_previous_file() {
    let directory = directory("atomic-replace");
    let path = directory.join("data");

    write_atomic(&path, b"first").unwrap();
    write_atomic(&path, b"second").unwrap();

    assert_eq!(read_private_bounded(&path, 64).unwrap(), b"second");
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn appended_writes_accumulate_across_reopens() {
    let directory = directory("append-accumulate");
    let path = directory.join("trace.jsonl");

    open_private_append(&path)
        .unwrap()
        .write_all(b"one\n")
        .unwrap();
    open_private_append(&path)
        .unwrap()
        .write_all(b"two\n")
        .unwrap();

    assert_eq!(read_private_bounded(&path, 64).unwrap(), b"one\ntwo\n");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn private_lock_creates_and_reopens_the_lock_file() {
    let directory = directory("lock-create");
    let path = directory.join("lock");

    drop(open_private_lock(&path, true).unwrap());

    assert!(path.is_file());
    drop(open_private_lock(&path, false).unwrap());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn user_write_accepts_a_bare_relative_filename() {
    let directory = directory("bare-relative");
    let guard = CurrentDirectoryGuard::enter(&directory);

    write_user_atomic(Path::new("bare-name.txt"), b"relative").unwrap();

    assert_eq!(std::fs::read("bare-name.txt").unwrap(), b"relative");
    assert!(directory.join("bare-name.txt").is_file());
    drop(guard);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn ensure_private_creates_the_nested_parent_chain() {
    let directory = directory("nested-parents");
    let nested = directory.join("outer").join("inner");

    crate::storage::private_file_parent::ensure_private(&nested).unwrap();

    assert!(nested.is_dir());
    write_atomic(&nested.join("data"), b"nested").unwrap();
    assert_eq!(
        read_private_bounded(&nested.join("data"), 16).unwrap(),
        b"nested"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
fn shared_directory(guard: &crate::refs::refs_test_support::HomeGuard) -> PathBuf {
    let shared = guard.path().join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o755)).unwrap();
    shared
}

#[cfg(unix)]
#[test]
fn a_lock_in_a_directory_others_can_reach_names_that_directory() {
    let guard = crate::refs::refs_test_support::HomeGuard::new();
    let shared = shared_directory(&guard);
    let deadline = crate::Deadline::after(2_000).unwrap();

    let error = crate::FileLock::acquire(&shared.join("refs.lock"), deadline, "test")
        .err()
        .expect("a shared directory is refused");

    assert_eq!(error.code, crate::ErrorCode::PermDenied);
    assert!(error.suggestion.unwrap().contains("chmod 700"));
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "state_directory_permission");
    assert_eq!(details["path"], shared.display().to_string());
}

#[cfg(unix)]
#[test]
fn a_lock_file_others_can_read_names_that_file() {
    let guard = crate::refs::refs_test_support::HomeGuard::new();
    let private = guard.path().join("private");
    std::fs::create_dir_all(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    let lock = private.join("refs.lock");
    std::fs::write(&lock, b"").unwrap();
    std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o644)).unwrap();
    let deadline = crate::Deadline::after(2_000).unwrap();

    let error = crate::FileLock::acquire(&lock, deadline, "test")
        .err()
        .expect("a shared file is refused");

    assert_eq!(error.code, crate::ErrorCode::PermDenied);
    assert!(error.suggestion.unwrap().contains("chmod 600"));
    assert_eq!(error.details.unwrap()["path"], lock.display().to_string());
}

#[cfg(unix)]
#[test]
fn a_state_file_write_into_a_directory_others_can_reach_is_perm_denied() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let file = crate::storage::state_file::StateFile::under_state_root(&["web", "a.json"]).unwrap();
    let directory = file.path().parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();

    let error = file
        .write(b"{}")
        .expect_err("a shared directory is refused");

    assert_eq!(error.code, crate::ErrorCode::PermDenied);
    assert_eq!(
        error.details.unwrap()["path"],
        directory.display().to_string()
    );
}
