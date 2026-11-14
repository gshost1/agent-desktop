use super::{DaemonEndpoint, route_values};
use std::path::Path;

fn argv(args: &[&str]) -> Vec<String> {
    std::iter::once("agent-desktop")
        .chain(args.iter().copied())
        .map(str::to_owned)
        .collect()
}

#[test]
fn route_values_read_session_and_agent_id_in_both_spellings() {
    let args = argv(&["click", "@s1:e1", "--session", "alpha", "--agent-id=worker-2"]);

    assert_eq!(route_values(&args), vec!["alpha", "worker-2"]);
}

#[test]
fn route_values_stop_at_the_argument_terminator() {
    let args = argv(&["type", "@s1:e1", "--", "--session", "x"]);

    assert!(route_values(&args).is_empty());
}

#[test]
fn endpoint_files_sit_beside_the_socket() {
    let endpoint =
        DaemonEndpoint::from_socket(Path::new("/r/daemon/0123456789abcdef.sock")).unwrap();

    assert_eq!(endpoint.dir(), Path::new("/r/daemon"));
    assert_eq!(endpoint.lock(), Path::new("/r/daemon/0123456789abcdef.lock"));
    assert_eq!(endpoint.state(), Path::new("/r/daemon/0123456789abcdef.state"));
}

#[test]
fn socket_paths_outside_a_daemon_directory_or_with_a_bad_key_are_refused() {
    for path in [
        "/r/other/0123456789abcdef.sock",
        "/r/daemon/0123456789abcdeg.sock",
        "/r/daemon/0123.sock",
        "/r/daemon/0123456789abcdef.lock",
        "r/daemon/0123456789abcdef.sock",
    ] {
        assert_eq!(DaemonEndpoint::from_socket(Path::new(path)), None, "{path}");
    }
}

#[test]
fn socket_path_too_long_for_the_platform_is_refused() {
    let long = format!("/{}/daemon/0123456789abcdef.sock", "d".repeat(90));

    assert_eq!(DaemonEndpoint::from_socket(Path::new(&long)), None);
}

#[test]
fn key_changes_with_build_session_and_environment_but_not_with_the_command() {
    use crate::daemon::env_subset::EnvSubset;
    use crate::daemon::request::{DaemonRequest, PROTOCOL_VERSION};
    let base = DaemonRequest {
        protocol: PROTOCOL_VERSION,
        build: "1.0.0:1:2".into(),
        argv: argv(&["status"]),
        cwd: "/".into(),
        umask: 0o022,
        env: EnvSubset::from_pairs(vec![("DISPLAY".into(), ":150".into())]),
    };
    let mut other_command = base.clone();
    other_command.argv = argv(&["snapshot", "--app", "X"]);
    other_command.cwd = "/tmp".into();
    let mut other_build = base.clone();
    other_build.build.push('9');
    let mut other_session = base.clone();
    other_session.argv = argv(&["status", "--session", "s2"]);
    let mut other_display = base.clone();
    other_display.env = EnvSubset::from_pairs(vec![("DISPLAY".into(), ":151".into())]);

    let key = super::key_for(&base);
    assert_eq!(key.len(), 16);
    assert_eq!(super::key_for(&other_command), key);
    assert_ne!(super::key_for(&other_build), key);
    assert_ne!(super::key_for(&other_session), key);
    assert_ne!(super::key_for(&other_display), key);
}
