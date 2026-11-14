use super::ServerState;
use crate::daemon::build_identity::BuildIdentity;
use crate::daemon::endpoint::DaemonEndpoint;
use crate::daemon::env_subset::EnvSubset;
use crate::daemon::request::{DaemonRequest, PROTOCOL_VERSION};

fn env(display: &str) -> EnvSubset {
    EnvSubset::from_pairs(vec![("DISPLAY".into(), display.into())])
}

fn server() -> ServerState {
    let metadata = std::fs::metadata(std::env::current_exe().unwrap()).unwrap();
    ServerState::new(
        DaemonEndpoint::from_socket(std::path::Path::new("/r/daemon/0123456789abcdef.sock"))
            .unwrap(),
        BuildIdentity::from_parts("9.9.9", &metadata),
        env(":150"),
    )
}

fn request() -> DaemonRequest {
    let metadata = std::fs::metadata(std::env::current_exe().unwrap()).unwrap();
    DaemonRequest {
        protocol: PROTOCOL_VERSION,
        build: BuildIdentity::from_parts("9.9.9", &metadata).as_str().to_owned(),
        argv: vec!["agent-desktop".into(), "status".into()],
        cwd: "/".into(),
        umask: 0o022,
        env: env(":150"),
    }
}

#[test]
fn matching_request_is_served() {
    let server = server();

    assert_eq!(server.mismatch(&request()), None);
}

#[test]
fn other_protocol_build_or_environment_is_refused() {
    let server = server();
    let mut protocol = request();
    protocol.protocol += 1;
    let mut build = request();
    build.build.push('x');
    let mut display = request();
    display.env = env(":151");

    assert_eq!(server.mismatch(&protocol), Some("protocol"));
    assert_eq!(server.mismatch(&build), Some("build"));
    assert_eq!(server.mismatch(&display), Some("environment"));
}

#[test]
fn execution_slot_is_held_by_one_command_at_a_time() {
    let server = server();

    assert!(server.claim());
    assert!(!server.claim());
    server.release();
    assert!(server.claim());
}

#[test]
fn connections_past_the_limit_are_refused() {
    let server = server();
    let opened = (0..80).filter(|_| server.open_connection()).count();
    server.close_connection();

    assert_eq!(opened, 64);
    assert!(server.open_connection());
}
