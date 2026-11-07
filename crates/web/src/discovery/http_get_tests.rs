use super::{response_complete, split_body};

#[test]
fn a_response_is_complete_once_content_length_bytes_arrive() {
    let partial = b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n{\"a\":";
    let full = b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n{\"a\":\"bc\"}";
    assert!(!response_complete(partial));
    assert!(response_complete(full));
    assert!(!response_complete(b"HTTP/1.1 200 OK\r\nContent-Le"));
}

#[test]
fn only_a_200_response_yields_a_body() {
    assert_eq!(
        split_body(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").as_deref(),
        Some("{}")
    );
    assert!(split_body(b"HTTP/1.1 404 Not Found\r\n\r\nnope").is_none());
    assert!(split_body(b"garbage").is_none());
}
