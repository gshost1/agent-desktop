//! Drives a real headless Chrome through the crate's public API. Ignored by
//! default; run with `cargo test -p agent-desktop-web --test live_chrome -- --ignored`.
//! Chrome runs headless with a throwaway profile under the target directory,
//! so it never touches the desktop or a real profile.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use agent_desktop_core::{AccessibilityNode, Deadline};
use agent_desktop_web::{
    CdpConnection, ElementToken, PageSession, WebSnapshot, attach_to_target, click, close_target,
    connect, create_background_tab, endpoint_from_port, endpoint_from_user_data_dir, fill,
    list_page_targets, navigate, press_key, read_element, select_option, set_checked,
    snapshot_page,
};

const CHROME: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const FORM: &str = r#"<!doctype html><html><head><title>Live form</title></head><body>
<h1>Signup</h1>
<form id="signup">
<label for="name">Name</label><input id="name" type="text">
<label for="color">Color</label><select id="color"><option value="red">Red</option><option value="green">Green</option></select>
<label><input id="subscribe" type="checkbox">Subscribe</label>
<button type="submit">Submit</button>
</form>
<p id="result">pending</p>
<iframe srcdoc="<button>Inner</button>"></iframe>
<script>
document.getElementById('signup').addEventListener('submit', function (event) {
  event.preventDefault();
  document.getElementById('result').textContent = 'done:' + document.getElementById('name').value
    + '|' + document.getElementById('color').value + '|' + document.getElementById('subscribe').checked;
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

fn launch() -> HeadlessChrome {
    let profile = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("web-live-chrome-{}", std::process::id()));
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
            "Chrome never wrote DevToolsActivePort"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    chrome
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

fn find<'a>(snapshot: &'a WebSnapshot, role: &str, name: &str) -> (&'a AccessibilityNode, i64) {
    let (node, origin) = snapshot
        .preorder()
        .into_iter()
        .find(|(node, _)| node.role == role && node.identity.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role} named {name}: {:#?}", snapshot.tree));
    let backend = origin.unwrap().backend_node_id;
    if let Some(token) = node.identity.retained_object.as_deref() {
        assert_eq!(ElementToken::parse(token).unwrap().backend_node_id, backend);
    }
    (node, backend)
}

fn open_form(connection: &mut CdpConnection) -> PageSession {
    let target_id = create_background_tab(connection, "about:blank", deadline()).unwrap();
    let listed = list_page_targets(connection, deadline()).unwrap();
    assert!(listed.iter().any(|page| page.target_id == target_id));
    let session = attach_to_target(connection, &target_id, deadline()).unwrap();
    navigate(connection, &session, &data_url(FORM), deadline()).unwrap();
    session
}

#[test]
#[ignore = "launches headless Chrome"]
fn fills_selects_checks_and_submits_a_form_in_a_background_tab() {
    let chrome = launch();
    let endpoint = endpoint_from_user_data_dir(&chrome.profile).unwrap();
    let active_port_before =
        std::fs::read_to_string(chrome.profile.join("DevToolsActivePort")).unwrap();
    let by_port = endpoint_from_port(endpoint.port(), deadline()).unwrap();
    assert_eq!(by_port.websocket_url, endpoint.websocket_url);
    let mut connection = connect(&endpoint, deadline()).unwrap();
    let session = open_form(&mut connection);

    let snapshot = snapshot_page(&mut connection, &session, deadline()).unwrap();
    if std::env::var_os("WEB_LIVE_DUMP").is_some() {
        println!("{}", serde_json::to_string_pretty(&snapshot.tree).unwrap());
        println!("{:?}", snapshot.unreadable_frames);
    }
    assert_eq!(snapshot.tree.role, "webarea");
    assert_eq!(snapshot.tree.identity.name.as_deref(), Some("Live form"));
    assert_eq!(snapshot.origins.len(), snapshot.preorder().len());
    let (name_field, name) = find(&snapshot, "textfield", "Name");
    assert_eq!(
        name_field.identity.native_id.as_ref().unwrap().value,
        "name"
    );
    let (_, color) = find(&snapshot, "combobox", "Color");
    let (_, subscribe) = find(&snapshot, "checkbox", "Subscribe");
    let (_, submit) = find(&snapshot, "button", "Submit");
    find(&snapshot, "button", "Inner");
    find(&snapshot, "heading", "Signup");

    fill(&mut connection, &session, name, "Adaa", deadline()).unwrap();
    press_key(&mut connection, &session, "Backspace", deadline()).unwrap();
    assert_eq!(
        read_element(&mut connection, &session, name, deadline())
            .unwrap()
            .value
            .as_deref(),
        Some("Ada")
    );
    assert_eq!(
        select_option(&mut connection, &session, color, "Green", deadline()).unwrap(),
        "green"
    );
    assert!(set_checked(&mut connection, &session, subscribe, true, deadline()).unwrap());
    click(&mut connection, &session, submit, deadline()).unwrap();

    let after = snapshot_page(&mut connection, &session, deadline()).unwrap();
    let (_, result) = find(&after, "statictext", "done:Ada|green|true");
    assert!(result > 0);
    let (checkbox, _) = find(&after, "checkbox", "Subscribe");
    assert!(
        checkbox
            .presentation
            .states
            .iter()
            .any(|state| state == "checked")
    );
    let (combo, _) = find(&after, "combobox", "Color");
    assert_eq!(combo.identity.value.as_deref(), Some("Green"));

    navigate(&mut connection, &session, "about:blank", deadline()).unwrap();
    let stale = read_element(&mut connection, &session, name, deadline()).unwrap_err();
    assert_eq!(stale.code, agent_desktop_core::ErrorCode::StaleRef);

    close_target(&mut connection, &session.target_id, deadline()).unwrap();
    assert_eq!(
        std::fs::read_to_string(chrome.profile.join("DevToolsActivePort")).unwrap(),
        active_port_before
    );
}
