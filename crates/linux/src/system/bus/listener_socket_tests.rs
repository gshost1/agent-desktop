use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use super::{SocketTarget, connect, target_of};

#[test]
fn bus_addresses_name_their_socket() {
    assert_eq!(
        target_of("unix:path=/run/user/1000/at-spi/bus_0,guid=0123456789abcdef0123456789abcdef"),
        Some(SocketTarget::Path(PathBuf::from(
            "/run/user/1000/at-spi/bus_0"
        )))
    );
    assert_eq!(
        target_of("unix:abstract=/tmp/dbus-AbCdEf,guid=0123456789abcdef0123456789abcdef"),
        Some(SocketTarget::Abstract(b"/tmp/dbus-AbCdEf".to_vec()))
    );
    assert_eq!(target_of("tcp:host=127.0.0.1,port=9"), None);
    assert_eq!(target_of("not an address"), None);
}

#[test]
fn a_path_socket_connects() {
    let dir = std::env::temp_dir().join(format!("ad-listener-socket-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bus");
    let listener = UnixListener::bind(&path).unwrap();
    futures_lite::future::block_on(connect(&SocketTarget::Path(path))).expect("connected");
    listener.accept().expect("accepted");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_abstract_socket_connects() {
    use std::os::linux::net::SocketAddrExt;
    let name = format!("ad-listener-abstract-{}", std::process::id());
    let address = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
    let listener = UnixListener::bind_addr(&address).unwrap();
    futures_lite::future::block_on(connect(&SocketTarget::Abstract(name.into_bytes())))
        .expect("connected");
    listener.accept().expect("accepted");
}

#[test]
fn a_missing_socket_is_an_error_and_not_a_hang() {
    let missing = std::env::temp_dir().join("ad-listener-missing-socket-path");
    assert!(futures_lite::future::block_on(connect(&SocketTarget::Path(missing))).is_err());
}

#[test]
fn an_abstract_name_may_fill_the_whole_path_and_a_file_path_leaves_room_for_its_nul() {
    use super::socket_address;
    assert!(socket_address(&SocketTarget::Abstract(vec![b'a'; 107])).is_ok());
    assert!(socket_address(&SocketTarget::Abstract(vec![b'a'; 108])).is_err());
    let path = |n| SocketTarget::Path(PathBuf::from("/".repeat(n)));
    assert!(socket_address(&path(107)).is_ok());
    assert!(socket_address(&path(108)).is_err());
}
