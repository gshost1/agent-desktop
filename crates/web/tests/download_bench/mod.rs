//! A real headless Chrome with its own registry, download ledger and
//! download folder under the target directory, shared by the live download
//! tests. The user's own downloads are never touched.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::common::{HeadlessChrome, launch_prepared};
use agent_desktop_core::system::cdp_endpoint::CdpEndpoint;
use agent_desktop_core::system::launch_options::LaunchOptions;
use agent_desktop_core::{
    Action, ActionRequest, AdapterError, Deadline, DownloadScope, InteractionLease, ObservationOps,
    ObservationRequest, ObservationRoot, OpenUrlRequest, ProcessId, ProcessIdentity, RefEntry,
    RefEntryIdentity, RefProcess, RefSource, SnapshotSurface, StateFile, SystemOps, TreeOptions,
    WindowInfo, WindowState, ref_action::execute_entry,
};
use agent_desktop_web::{SystemConnector, WebChannelAdapter, endpoint_from_user_data_dir};

/// One browser at a time: a parallel Chrome slows download starts past
/// the 200 ms a connection-scope click holds its connection open.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub const INSTANCE: &str = "live-downloads";
pub const PID: u32 = 4_242;
pub const CSV: &str = "id,name\n1,Ada\n2,Grace\n";

pub struct HeadlessPlatform;

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
pub struct Bench {
    _serial: std::sync::MutexGuard<'static, ()>,
    chrome: HeadlessChrome,
    root: PathBuf,
    pub downloads: PathBuf,
}

impl Bench {
    /// Starts the browser with its download folder in `scope`: for
    /// `Profile` the adapter writes the folder into the fresh profile before
    /// the browser starts, as `launch --cdp --download-dir` does. Files in
    /// `existing` are in the folder before launch.
    pub fn new(label: &str, scope: DownloadScope, existing: &[&str]) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("web-downloads-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        private(&root);
        let downloads = root.join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        for name in existing {
            std::fs::write(downloads.join(name), b"kept").unwrap();
        }
        let serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stray = root.join("browser-default");
        let chrome = launch_prepared(label, |profile| {
            let preferences = profile.join("Default").join("Preferences");
            std::fs::create_dir_all(preferences.parent().unwrap()).unwrap();
            let fallback = serde_json::json!({
                "download": { "default_directory": stray, "prompt_for_download": false }
            });
            std::fs::write(&preferences, fallback.to_string()).unwrap();
            if scope == DownloadScope::Profile {
                let options = LaunchOptions {
                    args: vec![format!("--user-data-dir={}", profile.display())],
                    ..LaunchOptions::default()
                };
                let prepared = WebChannelAdapter::with_connector(
                    HeadlessPlatform,
                    Box::new(SystemConnector::with_sources(None, Vec::new())),
                )
                .prepare_cdp_downloads(&options, &downloads.to_string_lossy())
                .unwrap();
                assert_eq!(prepared, DownloadScope::Profile);
            }
        });
        let bench = Self {
            _serial: serial,
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
                    download_scope: Some(scope),
                },
            )
            .unwrap();
        bench
    }

    pub fn process(&self) -> ProcessIdentity {
        ProcessIdentity::new(PID, INSTANCE)
    }

    /// A fresh adapter, as a new agent-desktop invocation would build.
    pub fn adapter(&self) -> WebChannelAdapter<HeadlessPlatform> {
        let registry = StateFile::at(self.root.join("endpoints.json"));
        let connector = SystemConnector::with_sources(Some(registry), Vec::new());
        WebChannelAdapter::with_connector(HeadlessPlatform, Box::new(connector))
            .with_download_ledger(Some(StateFile::at(self.root.join("downloads.json"))))
    }

    pub fn window(&self) -> WindowInfo {
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

    /// Opens the page, snapshots it and clicks its download link. Returns
    /// how long the click itself took.
    pub fn click_download(
        &self,
        adapter: &WebChannelAdapter<HeadlessPlatform>,
        port: u16,
    ) -> Duration {
        self.click_named(adapter, port, "Download report")
    }

    /// Opens the page, snapshots it and clicks the element named `name`.
    /// Returns how long the click itself took.
    pub fn click_named(
        &self,
        adapter: &WebChannelAdapter<HeadlessPlatform>,
        port: u16,
        name: &str,
    ) -> Duration {
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
        let link = find(&tree, name).expect("the page shows the element");
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
        let started = std::time::Instant::now();
        execute_entry(adapter, &entry, request).unwrap();
        started.elapsed()
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn private(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
}

/// A bench folder on Windows inherits the user profile's access list, which
/// already keeps it private.
#[cfg(not(unix))]
fn private(_dir: &Path) {}

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

pub fn deadline() -> Deadline {
    Deadline::after(10_000).unwrap()
}

/// Serves a page linking to `/report.csv`, which arrives as an attachment,
/// next to a button that does nothing; the body comes `body_delay` after the
/// headers.
pub fn serve(body_delay: Duration) -> u16 {
    serve_with(Duration::ZERO, body_delay)
}

/// [`serve`] with the attachment's headers sent `header_delay` after the
/// request, like a server that builds the file before it answers.
pub fn serve_with(header_delay: Duration, body_delay: Duration) -> u16 {
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
                    std::thread::sleep(header_delay);
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/csv\r\nContent-Disposition: attachment; filename=\"report.csv\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        CSV.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    std::thread::sleep(body_delay);
                    let _ = stream.write_all(CSV.as_bytes());
                } else {
                    let page = r#"<!doctype html><title>Reports</title><a href="/report.csv">Download report</a><button>Nothing</button>"#;
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

/// Asserts `path` is `name` in the download folder with the served content,
/// and that the folder holds exactly `names`.
pub fn assert_saved(bench: &Bench, path: &str, name: &str, names: &[&str]) {
    let saved = Path::new(path);
    assert_eq!(saved, bench.downloads.join(name));
    assert_eq!(std::fs::read_to_string(saved).unwrap(), CSV);
    let mut present: Vec<String> = std::fs::read_dir(&bench.downloads)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    present.sort();
    assert_eq!(present, names, "the folder holds only the saved files");
}
