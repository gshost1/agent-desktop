use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use agent_desktop_core::{Deadline, ErrorCode};
use tungstenite::Message;

use super::WebSocketTransport;
use crate::{CdpTransport, WebSocketUrl};

fn local_server() -> (TcpListener, WebSocketUrl) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    (
        listener,
        WebSocketUrl::loopback(port, "/devtools/browser/test").unwrap(),
    )
}

#[test]
fn multi_megabyte_messages_round_trip_and_pings_are_skipped() {
    let (listener, url) = local_server();
    let big = "x".repeat(8 * 1024 * 1024);
    let expected = big.clone();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        let request = socket.read().unwrap();
        socket.send(Message::Ping(vec![1].into())).unwrap();
        socket.send(Message::text(big)).unwrap();
        while socket.read().is_ok() {}
        request.into_text().unwrap().as_str().to_owned()
    });

    let deadline = Deadline::after(5_000).unwrap();
    let mut transport = WebSocketTransport::connect(&url, deadline).unwrap();
    transport.send_text("{\"id\":1}", deadline).unwrap();
    let received = transport.receive_text(deadline).unwrap();

    assert_eq!(received.len(), expected.len());
    drop(transport);
    assert_eq!(server.join().unwrap(), "{\"id\":1}");
}

#[test]
fn a_refused_upgrade_is_a_permission_error() {
    let (listener, url) = local_server();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = [0_u8; 2048];
        let _ = stream.read(&mut buffer).unwrap();
        stream
            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
    });

    let error = WebSocketTransport::connect(&url, Deadline::after(2_000).unwrap()).unwrap_err();

    server.join().unwrap();
    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(error.details.as_ref().unwrap()["http_status"], 403);
}

#[test]
fn a_held_upgrade_times_out_at_the_deadline() {
    let (listener, url) = local_server();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        thread::sleep(Duration::from_millis(600));
        drop(stream);
    });

    let started = Instant::now();
    let error = WebSocketTransport::connect(&url, Deadline::after(200).unwrap()).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(started.elapsed() < Duration::from_millis(550));
    server.join().unwrap();
}

#[test]
fn nothing_listening_is_an_unavailable_endpoint() {
    let (listener, url) = local_server();
    drop(listener);
    let error = WebSocketTransport::connect(&url, Deadline::after(1_000).unwrap()).unwrap_err();
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "cdp_endpoint_unavailable"
    );
}

#[test]
fn a_silent_server_read_is_bounded_by_the_deadline() {
    let (listener, url) = local_server();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let socket = tungstenite::accept(stream).unwrap();
        thread::sleep(Duration::from_millis(600));
        drop(socket);
    });

    let mut transport = WebSocketTransport::connect(&url, Deadline::after(1_000).unwrap()).unwrap();
    let started = Instant::now();
    let error = transport
        .receive_text(Deadline::after(150).unwrap())
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
    assert!(started.elapsed() < Duration::from_millis(500));
    server.join().unwrap();
}
