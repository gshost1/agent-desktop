use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use zbus::connection::Builder;

use super::{children_are_hidden, is_hidden_state};
use crate::tree::AtspiElement;
use crate::tree::vocabulary::state_bits::{EXPANDED, SHOWING, VISIBLE};
use crate::tree::walk::walker_expand::UNKNOWN_PID;
use agent_desktop_core::Deadline;

enum StateReply {
    Words(Vec<u32>),
    Error,
    Pending,
}

struct Accessible {
    reply: StateReply,
    calls: Arc<AtomicUsize>,
}

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Accessible {
    async fn get_state(&self) -> zbus::fdo::Result<Vec<u32>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.reply {
            StateReply::Words(words) => Ok(words.clone()),
            StateReply::Error => Err(zbus::fdo::Error::Failed("GetState failed".into())),
            StateReply::Pending => std::future::pending().await,
        }
    }
}

struct Fixture {
    client: zbus::Connection,
    _server: zbus::Connection,
    children: Vec<AtspiElement>,
    calls: Arc<AtomicUsize>,
}

fn fixture(replies: Vec<StateReply>) -> Fixture {
    let calls = Arc::new(AtomicUsize::new(0));
    let (client, server) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut server = Builder::async_io_unix_stream(server)
        .server(zbus::Guid::generate())
        .unwrap()
        .p2p();
    let mut children = Vec::new();
    for (index, reply) in replies.into_iter().enumerate() {
        let path = format!("/child{index}");
        children.push(AtspiElement::new(":1.1", &path, std::process::id()));
        server = server
            .serve_at(
                path,
                Accessible {
                    reply,
                    calls: Arc::clone(&calls),
                },
            )
            .unwrap();
    }
    let (client, server) = zbus::block_on(futures_lite::future::zip(
        Builder::async_io_unix_stream(client).p2p().build(),
        server.build(),
    ));
    Fixture {
        client: client.unwrap(),
        _server: server.unwrap(),
        children,
        calls,
    }
}

#[test]
fn missing_state_is_not_hidden() {
    assert!(!is_hidden_state(None));
}

#[test]
fn empty_state_is_not_hidden() {
    assert!(!is_hidden_state(Some(&[])));
}

#[test]
fn showing_state_is_not_hidden() {
    assert!(!is_hidden_state(Some(&[1 << SHOWING, 0])));
}

#[test]
fn expanded_without_showing_is_not_hidden() {
    assert!(!is_hidden_state(Some(&[1 << EXPANDED, 0])));
}

#[test]
fn state_without_showing_or_expanded_is_hidden() {
    assert!(is_hidden_state(Some(&[1 << VISIBLE, 0])));
    assert!(is_hidden_state(Some(&[0, 0])));
}

#[test]
fn get_state_error_keeps_children() {
    let fixture = fixture(vec![StateReply::Error]);
    assert!(!children_are_hidden(
        &fixture.client,
        &fixture.children,
        Deadline::after(5_000).unwrap(),
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn get_state_timeout_keeps_children() {
    let fixture = fixture(vec![StateReply::Pending]);
    assert!(!children_are_hidden(
        &fixture.client,
        &fixture.children,
        Deadline::after(1_000).unwrap(),
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn expired_deadline_keeps_children_without_fetching_state() {
    let fixture = fixture(vec![StateReply::Words(vec![0, 0])]);
    assert!(!children_are_hidden(
        &fixture.client,
        &fixture.children,
        Deadline::after(0).unwrap(),
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn unknown_pid_keeps_children_without_fetching_state() {
    let mut fixture = fixture(vec![StateReply::Words(vec![0, 0])]);
    fixture.children[0] = AtspiElement::new(":1.1", "/child0", UNKNOWN_PID);
    assert!(!children_are_hidden(
        &fixture.client,
        &fixture.children,
        Deadline::after(5_000).unwrap(),
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn all_hidden_children_are_hidden() {
    let fixture = fixture(vec![
        StateReply::Words(vec![0, 0]),
        StateReply::Words(vec![1 << VISIBLE, 0]),
    ]);
    assert!(children_are_hidden(
        &fixture.client,
        &fixture.children,
        Deadline::after(5_000).unwrap(),
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn one_showing_child_keeps_children() {
    let fixture = fixture(vec![
        StateReply::Words(vec![0, 0]),
        StateReply::Words(vec![1 << SHOWING, 0]),
    ]);
    assert!(!children_are_hidden(
        &fixture.client,
        &fixture.children,
        Deadline::after(5_000).unwrap(),
    ));
}

#[test]
fn no_children_are_hidden() {
    let fixture = fixture(Vec::new());
    assert!(children_are_hidden(
        &fixture.client,
        &fixture.children,
        Deadline::after(5_000).unwrap(),
    ));
    assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
}
