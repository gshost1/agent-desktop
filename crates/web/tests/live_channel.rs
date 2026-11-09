//! Drives a real headless Chrome through `WebChannelAdapter` and core's ref
//! action pipeline: open-url, a web-surface snapshot, and set-value, select,
//! check and click on the refs it returns. Ignored by default; run with
//! `cargo test -p agent-desktop-web --test live_channel -- --ignored`.
//! Chrome runs headless with a throwaway profile under the target directory.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use agent_desktop_core::{
    AccessibilityNode, Action, ActionRequest, ActionResult, AdapterError, Deadline,
    DeliverySemantics, InteractionLease, ObservationOps, ObservationRequest, ObservationRoot,
    OpenUrlRequest, ProcessId, ProcessIdentity, RefCapabilities, RefEntry, RefEntryIdentity,
    RefGeometry, RefProcess, RefScope, RefSource, SnapshotSurface, SystemOps, TreeOptions,
    WindowInfo, WindowState, ref_action::execute_entry,
};
use agent_desktop_web::{SystemConnector, WebChannelAdapter};

const CHROME: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const INSTANCE: &str = "live";

const FORM: &str = r#"<!doctype html><html><head><title>Channel form</title></head><body>
<form id="f">
<label for="name">Name</label><input id="name" type="text">
<label for="color">Color</label><select id="color"><option value="red">Red</option><option value="green">Green</option></select>
<label><input id="sub" type="checkbox">Subscribe</label>
<button type="submit">Submit</button>
</form>
<p>pending</p>
<script>
document.getElementById('f').addEventListener('submit', function (event) {
  event.preventDefault();
  document.querySelector('p').textContent = 'done:' + document.getElementById('name').value
    + '|' + document.getElementById('color').value + '|' + document.getElementById('sub').checked;
});
</script></body></html>"#;

struct HeadlessChrome {
    child: Child,
    profile: PathBuf,
}

impl Drop for HeadlessChrome {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

fn launch(label: &str) -> HeadlessChrome {
    let profile = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("web-live-channel-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&profile);
    std::fs::create_dir_all(&profile).unwrap();
    let child = Command::new(CHROME)
        .arg("--headless=new")
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg("--remote-debugging-port=0")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("about:blank")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let chrome = HeadlessChrome { child, profile };
    let started = Instant::now();
    while !chrome.profile.join("DevToolsActivePort").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "no DevToolsActivePort"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    chrome
}

/// The platform side of the test: it owns every port the browser listens
/// on, grants interaction leases, and knows nothing else.
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

fn deadline() -> Deadline {
    Deadline::after(10_000).unwrap()
}

fn data_url(html: &str) -> String {
    let mut url = String::from("data:text/html;charset=utf-8,");
    for byte in html.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

fn window(pid: u32) -> WindowInfo {
    WindowInfo {
        id: "headless".into(),
        title: String::new(),
        app: "Google Chrome".into(),
        pid: ProcessId::new(pid),
        process_instance: Some(INSTANCE.into()),
        bounds: None,
        state: WindowState::default(),
    }
}

fn snapshot(
    adapter: &WebChannelAdapter<HeadlessPlatform>,
    window: &WindowInfo,
) -> AccessibilityNode {
    let options = TreeOptions {
        surface: SnapshotSurface::Web,
        ..TreeOptions::default()
    };
    let observed = adapter
        .observe_tree(
            ObservationRoot::Window(window),
            &ObservationRequest::snapshot(&options, deadline()),
        )
        .unwrap();
    assert_eq!(observed.channel().unwrap().channel, "cdp");
    observed.into_accessibility_tree().unwrap()
}

fn find<'a>(node: &'a AccessibilityNode, role: &str, name: &str) -> Option<&'a AccessibilityNode> {
    if node.role == role && node.identity.name.as_deref() == Some(name) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| find(child, role, name))
}

fn texts(node: &AccessibilityNode, out: &mut Vec<String>) {
    out.extend(node.identity.name.clone());
    for child in &node.children {
        texts(child, out);
    }
}

fn entry(tree: &AccessibilityNode, role: &str, name: &str, pid: u32) -> RefEntry {
    let node = find(tree, role, name).unwrap_or_else(|| panic!("no {role} {name}"));
    RefEntry {
        process: RefProcess {
            pid: ProcessId::new(pid),
            process_instance: Some(INSTANCE.into()),
        },
        identity: RefEntryIdentity {
            retained_object: node.identity.retained_object.clone(),
            role: node.role.clone(),
            name: node.identity.name.clone(),
            capture: agent_desktop_core::CaptureVerdict::Unique,
            ..RefEntryIdentity::default()
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: RefCapabilities {
            states: node.presentation.states.clone(),
            available_actions: node.presentation.available_actions.clone(),
        },
        source: RefSource {
            source_surface: SnapshotSurface::Web,
            ..RefSource::default()
        },
        scope: RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: Default::default(),
        },
    }
}

fn act(
    adapter: &WebChannelAdapter<HeadlessPlatform>,
    entry: &RefEntry,
    action: Action,
) -> ActionResult {
    let request = ActionRequest::headless(action.clone()).with_timeout_ms(Some(10_000));
    execute_entry(adapter, entry, request).unwrap_or_else(|error| panic!("{action:?}: {error:?}"))
}

#[test]
#[ignore = "launches headless Chrome"]
fn the_unified_loop_drives_a_page_through_core() {
    let chrome = launch("loop");
    let pid = chrome.child.id();
    let connector = SystemConnector::with_sources(None, vec![chrome.profile.clone()]);
    let adapter = WebChannelAdapter::with_connector(HeadlessPlatform, Box::new(connector));
    let window = window(pid);
    let url = data_url(FORM);
    let opened = adapter
        .open_url(&OpenUrlRequest {
            process: ProcessIdentity::new(pid, INSTANCE),
            window: Some(&window),
            url: &url,
            new_tab: false,
            deadline: deadline(),
        })
        .unwrap();
    assert_eq!(opened.channel, "cdp");
    assert_eq!(opened.title, "Channel form");
    assert_eq!(opened.opened, "current_tab");

    let tree = snapshot(&adapter, &window);
    let typed = act(
        &adapter,
        &entry(&tree, "textfield", "Name", pid),
        Action::SetValue("Ada".into()),
    );
    assert_eq!(typed.disposition(), DeliverySemantics::delivered_verified());
    let picked = act(
        &adapter,
        &entry(&tree, "combobox", "Color", pid),
        Action::Select("Green".into()),
    );
    assert_eq!(
        picked.disposition(),
        DeliverySemantics::delivered_verified()
    );
    let checked = act(
        &adapter,
        &entry(&tree, "checkbox", "Subscribe", pid),
        Action::Check,
    );
    assert_eq!(
        checked.disposition(),
        DeliverySemantics::delivered_verified()
    );
    act(
        &adapter,
        &entry(&tree, "button", "Submit", pid),
        Action::Click,
    );

    let mut seen = Vec::new();
    let started = Instant::now();
    while !seen.iter().any(|text| text == "done:Ada|green|true") {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "page shows {seen:?}"
        );
        seen.clear();
        texts(&snapshot(&adapter, &window), &mut seen);
    }
}

#[test]
#[ignore = "launches headless Chrome"]
fn open_url_gives_a_browser_with_no_page_a_background_tab() {
    let chrome = launch("empty");
    let pid = chrome.child.id();
    let endpoint = agent_desktop_web::endpoint_from_user_data_dir(&chrome.profile).unwrap();
    let mut connection = agent_desktop_web::connect(&endpoint, deadline()).unwrap();
    for page in agent_desktop_web::list_page_targets(&mut connection, deadline()).unwrap() {
        agent_desktop_web::close_target(&mut connection, &page.target_id, deadline()).unwrap();
    }
    let started = Instant::now();
    while !agent_desktop_web::list_page_targets(&mut connection, deadline())
        .unwrap()
        .is_empty()
    {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "pages did not close"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let connector = SystemConnector::with_sources(None, vec![chrome.profile.clone()]);
    let adapter = WebChannelAdapter::with_connector(HeadlessPlatform, Box::new(connector));
    let window = window(pid);
    let url = data_url(FORM);
    let opened = adapter
        .open_url(&OpenUrlRequest {
            process: ProcessIdentity::new(pid, INSTANCE),
            window: Some(&window),
            url: &url,
            new_tab: false,
            deadline: deadline(),
        })
        .unwrap();
    assert_eq!(opened.opened, "new_tab");
    assert_eq!(opened.title, "Channel form");
    let pages = agent_desktop_web::list_page_targets(&mut connection, deadline()).unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].target_id, opened.target_id);
}
