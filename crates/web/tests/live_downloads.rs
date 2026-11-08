//! Downloads a file from a local server in a real headless Chrome through
//! `WebChannelAdapter`, and checks that `wait --download` returns a path
//! where the file exists. Ignored by default; run with
//! `cargo test -p agent-desktop-web --test live_downloads -- --ignored`.
//! Each test uses a throwaway profile, registry, ledger and download folder
//! under the target directory, never the user's own downloads.

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::{
    Action, ActionRequest, AdapterError, Deadline, InteractionLease, ObservationOps,
    ObservationRequest, ObservationRoot, OpenUrlRequest, ProcessId, ProcessIdentity, RefEntry,
    RefEntryIdentity, RefProcess, RefSource, SnapshotSurface, StateFile, SystemOps, TreeOptions,
    WindowInfo, WindowState, ref_action::execute_entry,
};
use agent_desktop_web::{SystemConnector, WebChannelAdapter, endpoint_from_user_data_dir};
use common::{HeadlessChrome, launch};

const INSTANCE: &str = "live-downloads";
const PID: u32 = 4_242;
const CSV: &str = "id,name\n1,Ada\n2,Grace\n";

struct HeadlessPlatform;

impl ObservationOps for HeadlessPlatform {}
impl agent_desktop_core::ActionOps for HeadlessPlatform {}
impl agent_desktop_core::InputOps for HeadlessPlatform {}
impl SystemOps for HeadlessPlatform {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        InteractionLease::guarded(deadline, ())
    }

    fn process_owns_tcp_listener(
        &self,
        _process: ProcessIdentity,
        _port: u16,
        _deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        Ok(true)
    }
}

/// One browser with its own registry, ledger and download folder. The test
/// platform owns every port, so a fixed pid names the browser process.
struct Bench {
    chrome: HeadlessChrome,
    root: PathBuf,
    downloads: PathBuf,
}

impl Bench {
    fn new(label: &str) -> Self {
        let chrome = launch(label);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("web-downloads-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        private(&root);
        let downloads = root.join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        let bench = Self {
            chrome,
            root,
            downloads,
        };
        let endpoint = endpoint_from_user_data_dir(&bench.chrome.profile).unwrap();
        bench
            .adapter()
            .remember_cdp_endpoint(
                bench.process(),
                &CdpEndpoint {
                    port: endpoint.port(),
                    http_endpoint: format!("http://127.0.0.1:{}", endpoint.port()),
                    websocket_url: endpoint.websocket_url.as_url(),
                    product: None,
                    download_dir: Some(bench.downloads.to_string_lossy().into_owned()),
                },
            )
            .unwrap();
        bench
    }

    fn process(&self) -> ProcessIdentity {
        ProcessIdentity::new(PID, INSTANCE)
    }

    /// A fresh adapter, as a new agent-desktop invocation would build.
    fn adapter(&self) -> WebChannelAdapter<HeadlessPlatform> {
        let registry = StateFile::at(self.root.join("endpoints.json"));
        let connector = SystemConnector::with_sources(Some(registry), Vec::new());
        WebChannelAdapter::with_connector(HeadlessPlatform, Box::new(connector))
            .with_download_ledger(Some(StateFile::at(self.root.join("downloads.json"))))
    }

    fn window(&self) -> WindowInfo {
        WindowInfo {
            id: "headless".into(),
            title: String::new(),
            app: "Google Chrome".into(),
            pid: ProcessId::new(PID),
            process_instance: Some(INSTANCE.into()),
            bounds: None,
            state: WindowState::default(),
        }
    }

    /// Opens the page, snapshots it and clicks its download link.
    fn click_download(&self, adapter: &WebChannelAdapter<HeadlessPlatform>, port: u16) {
        let window = self.window();
        adapter
            .open_url(&OpenUrlRequest {
                process: self.process(),
                window: Some(&window),
                url: &format!("http://127.0.0.1:{port}/"),
                new_tab: false,
                deadline: deadline(),
            })
            .unwrap();
        let options = TreeOptions {
            surface: SnapshotSurface::Web,
            ..TreeOptions::default()
        };
        let tree = adapter
            .observe_tree(
                ObservationRoot::Window(&window),
                &ObservationRequest::snapshot(&options, deadline()),
            )
            .unwrap()
            .into_accessibility_tree()
            .unwrap();
        let link = find(&tree, "Download report").expect("the page shows its download link");
        let entry = RefEntry {
            process: RefProcess {
                pid: ProcessId::new(PID),
                process_instance: Some(INSTANCE.into()),
            },
            identity: RefEntryIdentity {
                retained_object: link.identity.retained_object.clone(),
                role: link.role.clone(),
                name: link.identity.name.clone(),
                unique_at_capture: true,
                ..RefEntryIdentity::default()
            },
            source: RefSource {
                source_surface: SnapshotSurface::Web,
                ..RefSource::default()
            },
            ..test_entry_defaults()
        };
        let request = ActionRequest::headless(Action::Click).with_timeout_ms(Some(10_000));
        execute_entry(adapter, &entry, request).unwrap();
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn private(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn test_entry_defaults() -> RefEntry {
    RefEntry {
        process: RefProcess {
            pid: ProcessId::new(0),
            process_instance: None,
        },
        identity: RefEntryIdentity::default(),
        geometry: agent_desktop_core::RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: agent_desktop_core::RefCapabilities {
            states: Vec::new(),
            available_actions: vec!["Click".into()],
        },
        source: RefSource::default(),
        scope: agent_desktop_core::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: Default::default(),
        },
    }
}

fn find<'a>(
    node: &'a agent_desktop_core::AccessibilityNode,
    name: &str,
) -> Option<&'a agent_desktop_core::AccessibilityNode> {
    if node.identity.name.as_deref() == Some(name) && node.identity.retained_object.is_some() {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, name))
}

fn deadline() -> Deadline {
    Deadline::after(10_000).unwrap()
}

/// Serves a page linking to `/report.csv`, which arrives as an attachment;
/// the body comes `body_delay` after the headers.
fn serve(body_delay: Duration) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut request = Vec::new();
                let mut chunk = [0_u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => return,
                        Ok(read) => request.extend_from_slice(&chunk[..read]),
                    }
                }
                if request.starts_with(b"GET /report.csv") {
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\nContent-Disposition: attachment; filename=\"report.csv\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        CSV.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    std::thread::sleep(body_delay);
                    let _ = stream.write_all(CSV.as_bytes());
                } else {
                    let page = r#"<!doctype html><title>Reports</title><a href="/report.csv">Download report</a>"#;
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        page.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(page.as_bytes());
                }
            });
        }
    });
    port
}

fn assert_saved(bench: &Bench, path: &str) {
    let saved = Path::new(path);
    assert_eq!(saved, bench.downloads.join("report.csv"));
    assert_eq!(std::fs::read_to_string(saved).unwrap(), CSV);
    let names: Vec<_> = std::fs::read_dir(&bench.downloads)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, vec!["report.csv"], "only the placed file remains");
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_download_that_finished_before_a_later_command_waits_is_returned_where_it_exists() {
    let bench = Bench::new("dl-finished");
    let port = serve(Duration::ZERO);
    bench.click_download(&bench.adapter(), port);
    std::thread::sleep(Duration::from_millis(500));

    let info = bench
        .adapter()
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    assert_eq!(info.state, "completed");
    assert_eq!(info.bytes, CSV.len() as u64);
    assert_eq!(info.suggested_filename, "report.csv");
    assert_eq!(info.url, format!("http://127.0.0.1:{port}/report.csv"));
    assert_saved(&bench, &info.path);
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_download_still_receiving_when_a_later_command_waits_is_followed_on_disk() {
    let bench = Bench::new("dl-slow");
    let port = serve(Duration::from_millis(1_500));
    bench.click_download(&bench.adapter(), port);

    let info = bench
        .adapter()
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    assert_eq!(info.bytes, CSV.len() as u64);
    assert_saved(&bench, &info.path);
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_click_and_a_wait_in_one_invocation_follow_the_download_events() {
    let bench = Bench::new("dl-batch");
    let port = serve(Duration::from_millis(300));
    let adapter = bench.adapter();
    bench.click_download(&adapter, port);

    let info = adapter
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    assert_saved(&bench, &info.path);
}

#[test]
#[ignore = "launches headless Chrome"]
fn open_url_on_a_file_url_records_the_download_for_a_later_wait() {
    let bench = Bench::new("dl-open-url");
    let port = serve(Duration::ZERO);
    let window = bench.window();
    let opened = bench
        .adapter()
        .open_url(&OpenUrlRequest {
            process: bench.process(),
            window: Some(&window),
            url: &format!("http://127.0.0.1:{port}/report.csv"),
            new_tab: false,
            deadline: deadline(),
        })
        .unwrap();
    assert_eq!(opened.opened, "download");

    let info = bench
        .adapter()
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    assert_saved(&bench, &info.path);
}
