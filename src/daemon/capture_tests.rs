use super::OutputCapture;
use crate::daemon::endpoint::DaemonEndpoint;

fn write_fd(fd: i32, bytes: &[u8]) {
    let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
    assert_eq!(usize::try_from(written).unwrap(), bytes.len());
}

#[test]
fn descriptor_writes_are_captured_per_stream_and_the_files_are_gone() {
    let root = std::env::temp_dir().join(format!("ad-capture-{}", std::process::id()));
    let dir = root.join("daemon");
    crate::daemon::private_dir::ensure_private_dir(&dir).unwrap();
    let endpoint = DaemonEndpoint::from_socket(&dir.join("0123456789abcdef.sock")).unwrap();

    let capture = OutputCapture::begin(&endpoint).unwrap();
    write_fd(1, b"to-stdout-marker\n");
    write_fd(2, b"to-stderr-marker\n");
    let (out, err) = capture.finish().unwrap();
    let left = std::fs::read_dir(&dir).unwrap().count();
    std::fs::remove_dir_all(&root).unwrap();

    let out = String::from_utf8_lossy(&out);
    let err = String::from_utf8_lossy(&err);
    assert!(out.contains("to-stdout-marker") && !out.contains("to-stderr-marker"));
    assert!(err.contains("to-stderr-marker") && !err.contains("to-stdout-marker"));
    assert_eq!(left, 0);
}
