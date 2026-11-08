use std::io::Write;

use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::{Deadline, DownloadScope, ProcessIdentity, StateFile};

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
        download_dir: None,
        download_scope: None,
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
            websocket_url: "ws://127.0.0.1:9222/devtools/browser/abc".into(),
            download_folder: None,
        }
    );
}

#[test]
fn a_remembered_download_folder_keeps_its_scope_and_an_unscoped_one_is_per_connection() {
    let dir = TestDir::new();
    let connector =
        SystemConnector::with_sources(Some(StateFile::at(dir.path().join("e.json"))), Vec::new());
    let deadline = Deadline::after(1_000).unwrap();
    let scoped = ProcessIdentity::new(7u32, "scoped");
    let unscoped = ProcessIdentity::new(8u32, "unscoped");
    let mut profile = endpoint(9222);
    profile.download_dir = Some("/tmp/downloads".into());
    profile.download_scope = Some(DownloadScope::Profile);
    let mut legacy = endpoint(9333);
    legacy.download_dir = Some("/tmp/older".into());
    connector.remember(&scoped, &profile, deadline).unwrap();
    connector.remember(&unscoped, &legacy, deadline).unwrap();

    let folder = |process| connector.candidates(process)[0].download_folder().unwrap();

    assert_eq!(folder(&scoped).scope, DownloadScope::Profile);
    assert_eq!(
        folder(&scoped).path,
        std::path::PathBuf::from("/tmp/downloads")
    );
    assert_eq!(folder(&unscoped).scope, DownloadScope::Connection);
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
            user_data_dir: profile
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
    let dir = TestDir::new();
    let connector = SystemConnector::with_sources(None, Vec::new());
    let candidate = crate::channel::endpoint_candidate::EndpointCandidate {
        port,
        origin: CandidateOrigin::ActivePort {
            user_data_dir: dir.path().to_path_buf(),
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
