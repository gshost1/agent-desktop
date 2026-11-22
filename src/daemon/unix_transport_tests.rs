use super::UnixTransport;
use crate::daemon::endpoint::DaemonEndpoint;
use crate::daemon::private_dir::ensure_private_dir;
use crate::daemon::transport::DaemonTransport;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;

fn endpoint(name: &str) -> (std::path::PathBuf, DaemonEndpoint) {
    let root = std::path::PathBuf::from(format!("/tmp/adut-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let dir = root.join("daemon");
    ensure_private_dir(&dir).unwrap();
    let socket = dir.join("0123456789abcdef.sock");
    (root, DaemonEndpoint::from_socket(&socket).unwrap())
}

#[test]
fn listener_binds_owner_only_and_serves_its_own_user() {
    let (root, endpoint) = endpoint("serve");
    let listener = UnixTransport.listen(&endpoint).unwrap();
    let mode = std::fs::metadata(endpoint.socket())
        .unwrap()
        .permissions()
        .mode();
    let mut client = UnixTransport.connect(&endpoint).unwrap();
    client.write_all(b"x").unwrap();
    let mut accepted = listener.accept_trusted().unwrap().unwrap();
    let mut byte = [0_u8; 1];
    accepted.read_exact(&mut byte).unwrap();
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(&byte, b"x");
}

#[test]
fn missing_socket_reads_as_absent_so_a_daemon_may_start() {
    let (root, endpoint) = endpoint("absent");
    let error = UnixTransport.connect(&endpoint).err().unwrap();
    std::fs::remove_dir_all(&root).unwrap();

    assert!(UnixTransport.is_absent(&error));
}

#[test]
fn a_file_squatting_on_the_socket_path_is_never_replaced() {
    let (root, endpoint) = endpoint("squat");
    std::fs::write(endpoint.socket(), b"not a socket").unwrap();
    let refused = UnixTransport.listen(&endpoint).is_err();
    let kept = std::fs::read(endpoint.socket()).unwrap();
    std::fs::remove_dir_all(&root).unwrap();

    assert!(refused);
    assert_eq!(kept, b"not a socket");
}

#[cfg(target_os = "linux")]
#[test]
fn a_socket_served_by_another_executable_is_refused_and_not_treated_as_absent() {
    let (root, endpoint) = endpoint("foreign");
    let mut server = std::process::Command::new("python3")
        .args([
            "-I",
            "-c",
            "import socket,sys,time\ns=socket.socket(socket.AF_UNIX)\ns.bind(sys.argv[1])\ns.listen(4)\nprint(flush=True)\nc=s.accept()\ntime.sleep(5)",
        ])
        .arg(endpoint.socket())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut ready = [0_u8; 1];
    server
        .stdout
        .take()
        .unwrap()
        .read_exact(&mut ready)
        .unwrap();
    let error = UnixTransport.connect(&endpoint).err();
    server.kill().unwrap();
    server.wait().unwrap();
    std::fs::remove_dir_all(&root).unwrap();

    let error = error.expect("a foreign daemon is refused");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(!UnixTransport.is_absent(&error));
}
