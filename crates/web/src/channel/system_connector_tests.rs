use std::io::Write;

use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::{Deadline, ProcessIdentity, StateFile};

use super::SystemConnector;
use crate::channel::candidate_origin::CandidateOrigin;
use crate::channel::cdp_connector::CdpConnector;
use crate::channel::test_dir::TestDir;

fn endpoint(port: u16) -> CdpEndpoint {
    CdpEndpoint {
        port,
        http_endpoint: format!("http://127.0.0.1:{port}"),
        websocket_url: format!("ws://127.0.0.1:{port}/devtools/browser/abc"),
        product: None,
    }
}

#[test]
fn registry_candidates_match_the_exact_process_instance_newest_first() {
    let dir = TestDir::new();
    let connector =
        SystemConnector::with_sources(Some(StateFile::at(dir.path().join("e.json"))), Vec::new());
    let deadline = Deadline::after(1_000).unwrap();
    let browser = ProcessIdentity::new(7u32, "inst-a");
    connector
        .remember(&browser, &endpoint(9222), deadline)
        .unwrap();
    connector
        .remember(
            &ProcessIdentity::new(7u32, "inst-old"),
            &endpoint(9333),
            deadline,
        )
        .unwrap();
    let candidates = connector.candidates(&browser);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].port, 9222);
    assert_eq!(
        candidates[0].origin,
        CandidateOrigin::Registry {
            websocket_url: "ws://127.0.0.1:9222/devtools/browser/abc".into()
        }
    );
}

#[test]
fn a_profile_active_port_file_is_a_candidate_and_is_left_untouched() {
    let dir = TestDir::new();
    let profile = dir.path().join("profile");
    std::fs::create_dir_all(&profile).unwrap();
    let file_path = profile.join("DevToolsActivePort");
    let mut file = std::fs::File::create(&file_path).unwrap();
    file.write_all(b"9555\n/devtools/browser/xyz\n").unwrap();
    let before = std::fs::read(&file_path).unwrap();
    let connector =
        SystemConnector::with_sources(None, vec![profile.clone(), dir.path().join("none")]);
    let candidates = connector.candidates(&ProcessIdentity::new(1u32, "x"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].port, 9555);
    assert_eq!(
        candidates[0].origin,
        CandidateOrigin::ActivePort {
            websocket_path: "/devtools/browser/xyz".into()
        }
    );
    assert_eq!(std::fs::read(&file_path).unwrap(), before);
}

#[test]
fn an_approval_gated_endpoint_is_refused_when_the_caller_may_not_prompt() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0u8; 1024];
            let _ = std::io::Read::read(&mut stream, &mut buffer);
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        }
    });
    let connector = SystemConnector::with_sources(None, Vec::new());
    let candidate = crate::channel::endpoint_candidate::EndpointCandidate {
        port,
        origin: CandidateOrigin::ActivePort {
            websocket_path: "/devtools/browser/x".into(),
        },
    };
    let error = connector
        .open(&candidate, Deadline::after(2_000).unwrap(), false)
        .err()
        .unwrap();
    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);
    assert_eq!(error.details.unwrap()["kind"], "cdp_approval_required");
    server.join().unwrap();
}

fn watched_port() -> (std::net::TcpListener, u16) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

fn never_contacted(listener: &std::net::TcpListener) -> bool {
    matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
}

#[test]
fn a_registry_url_on_another_port_than_the_checked_one_is_never_contacted() {
    let (elsewhere, elsewhere_port) = watched_port();
    let connector = SystemConnector::with_sources(None, Vec::new());
    let candidate = crate::channel::endpoint_candidate::EndpointCandidate {
        port: 9,
        origin: CandidateOrigin::Registry {
            websocket_url: format!("ws://127.0.0.1:{elsewhere_port}/devtools/browser/x"),
        },
    };
    let error = connector
        .open(&candidate, Deadline::after(1_000).unwrap(), true)
        .err()
        .unwrap();
    assert_eq!(error.details.unwrap()["kind"], "cdp_endpoint_mismatch");
    assert!(never_contacted(&elsewhere));
}

#[test]
fn an_active_port_file_rewritten_after_the_check_is_not_followed() {
    let checked = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let checked_port = checked.local_addr().unwrap().port();
    checked.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut served = 0;
        while served < 2 && std::time::Instant::now() < until {
            match checked.accept() {
                Ok((mut stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    let mut buffer = [0u8; 1024];
                    let _ = std::io::Read::read(&mut stream, &mut buffer);
                    let _ =
                        stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                    served += 1;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
    });
    let (planted, planted_port) = watched_port();
    let dir = TestDir::new();
    let file_path = dir.path().join("DevToolsActivePort");
    std::fs::write(
        &file_path,
        format!("{checked_port}\n/devtools/browser/real\n"),
    )
    .unwrap();
    let connector = SystemConnector::with_sources(None, vec![dir.path().to_path_buf()]);
    let candidates = connector.candidates(&ProcessIdentity::new(1u32, "x"));
    std::fs::write(
        &file_path,
        format!("{planted_port}\n/devtools/browser/fake\n"),
    )
    .unwrap();
    let _ = connector.open(&candidates[0], Deadline::after(2_000).unwrap(), true);
    assert!(never_contacted(&planted));
    server.join().unwrap();
}
