//! Drives a real headless Chrome through the crate's public API. Ignored by
//! default; run with `cargo test -p agent-desktop-web -- --ignored`.
//! Chrome runs headless with a throwaway profile under the target directory,
//! so it never touches the desktop or a real profile.

mod common;

use agent_desktop_core::{DeliverySemantics, ErrorCode};
use agent_desktop_web::{
    click, close_target, endpoint_from_port, endpoint_from_user_data_dir, fill, list_page_targets,
    navigate, press_key, read_element, select_option, set_checked, snapshot_page,
};
use common::{connect_to, data_url, deadline, find, launch, open_tab};

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

#[test]
#[ignore = "launches headless Chrome"]
fn fills_selects_checks_and_submits_a_form_in_a_background_tab() {
    let chrome = launch("form");
    let endpoint = endpoint_from_user_data_dir(&chrome.profile).unwrap();
    let active_port_before =
        std::fs::read_to_string(chrome.profile.join("DevToolsActivePort")).unwrap();
    let by_port = endpoint_from_port(endpoint.port(), deadline()).unwrap();
    assert_eq!(by_port.websocket_url, endpoint.websocket_url);
    let mut connection = connect_to(&chrome);
    let session = open_tab(&mut connection, &data_url(FORM));
    let listed = list_page_targets(&mut connection, deadline()).unwrap();
    assert!(
        listed
            .iter()
            .any(|page| page.target_id == session.target_id)
    );

    let snapshot = snapshot_page(&mut connection, &session, deadline()).unwrap();
    if std::env::var_os("WEB_LIVE_DUMP").is_some() {
        println!("{}", serde_json::to_string_pretty(&snapshot.tree).unwrap());
        println!("{:?}", snapshot.unreadable_frames);
    }
    assert_eq!(snapshot.tree.role, "webarea");
    assert_eq!(snapshot.tree.identity.name.as_deref(), Some("Live form"));
    assert_eq!(snapshot.origins.len(), snapshot.preorder().len());
    assert!(snapshot.unreadable_frames.is_empty());
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

    let filled = fill(&mut connection, &session, &name, "Adaa", deadline()).unwrap();
    assert!(filled.verified);
    assert_eq!(filled.delivery(), DeliverySemantics::delivered_verified());
    let refused = fill(&mut connection, &session, &subscribe, "x", deadline()).unwrap_err();
    assert_eq!(refused.code, ErrorCode::ActionNotSupported);
    press_key(&mut connection, &session, "Backspace", deadline()).unwrap();
    assert_eq!(
        read_element(&mut connection, &session, &name, deadline())
            .unwrap()
            .value
            .as_deref(),
        Some("Ada")
    );
    assert_eq!(
        select_option(&mut connection, &session, &color, "Green", deadline()).unwrap(),
        "green"
    );
    assert!(set_checked(&mut connection, &session, &subscribe, true, deadline()).unwrap());
    click(&mut connection, &session, &submit, deadline()).unwrap();

    let after = snapshot_page(&mut connection, &session, deadline()).unwrap();
    find(&after, "statictext", "done:Ada|green|true");
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
    let stale = read_element(&mut connection, &session, &name, deadline()).unwrap_err();
    assert_eq!(stale.code, ErrorCode::StaleRef);

    close_target(&mut connection, &session.target_id, deadline()).unwrap();
    let gone = click(&mut connection, &session, &submit, deadline()).unwrap_err();
    assert_eq!(gone.code, ErrorCode::WindowNotFound, "{gone:?}");
    assert_eq!(
        std::fs::read_to_string(chrome.profile.join("DevToolsActivePort")).unwrap(),
        active_port_before
    );
}
