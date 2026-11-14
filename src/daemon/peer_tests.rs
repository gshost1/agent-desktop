use super::is_trusted_peer;
use std::os::unix::net::UnixStream;

#[test]
fn same_user_same_executable_peer_is_trusted() {
    let (one, _two) = UnixStream::pair().unwrap();

    assert!(is_trusted_peer(&one));
}

#[cfg(target_os = "linux")]
#[test]
fn peer_running_another_executable_is_refused() {
    use std::os::unix::net::UnixListener;
    let dir = std::env::temp_dir().join(format!("ad-peer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("s.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let mut child = std::process::Command::new("python3")
        .args([
            "-I",
            "-c",
            "import socket,sys,time\ns=socket.socket(socket.AF_UNIX)\ns.connect(sys.argv[1])\ntime.sleep(5)",
        ])
        .arg(&path)
        .spawn()
        .unwrap();
    let (stream, _) = listener.accept().unwrap();
    let trusted = is_trusted_peer(&stream);
    child.kill().unwrap();
    child.wait().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(!trusted);
}
