use super::{read_frame, read_message, write_frame, write_message};
use crate::daemon::reply::DaemonReply;

#[test]
fn frames_round_trip_raw_bytes() {
    let mut buffer = Vec::new();
    write_frame(&mut buffer, b"\x00\xffbytes").unwrap();
    write_frame(&mut buffer, b"").unwrap();
    let mut input = buffer.as_slice();

    assert_eq!(read_frame(&mut input, 64).unwrap(), b"\x00\xffbytes");
    assert_eq!(read_frame(&mut input, 64).unwrap(), b"");
    assert!(read_frame(&mut input, 64).is_err());
}

#[test]
fn frame_over_the_limit_is_refused_before_it_is_read() {
    let mut buffer = Vec::new();
    write_frame(&mut buffer, &[7_u8; 65]).unwrap();

    assert!(read_frame(&mut buffer.as_slice(), 64).is_err());
}

#[test]
fn truncated_frame_is_an_error() {
    let mut buffer = Vec::new();
    write_frame(&mut buffer, b"0123456789").unwrap();
    buffer.truncate(8);

    assert!(read_frame(&mut buffer.as_slice(), 64).is_err());
}

#[test]
fn replies_round_trip_with_their_status_tag() {
    let reply = DaemonReply::Accepted {
        command: "click".into(),
        rerun_safe: false,
    };
    let mut buffer = Vec::new();
    write_message(&mut buffer, &reply).unwrap();

    assert!(String::from_utf8_lossy(&buffer).contains("\"status\":\"accepted\""));
    assert_eq!(
        read_message::<DaemonReply>(&mut buffer.as_slice(), 1024).unwrap(),
        reply
    );
}
