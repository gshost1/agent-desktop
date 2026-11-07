//! Live headless-Chrome checks for out-of-process iframes, deep documents and
//! document identity. Ignored by default; run with
//! `cargo test -p agent-desktop-web -- --ignored`.

mod common;

use agent_desktop_core::ErrorCode;
use agent_desktop_web::{
    attach_to_target, click, fill, navigate, read_element, snapshot_page, wait_for_load,
};
use common::{connect_to, data_url, deadline, find, launch, open_tab, serve};

const INNER: &str = r#"<!doctype html><html><head><title>Inner</title></head><body>
<button id="inner-go" onclick="document.getElementById('out').textContent='pressed'">Inner go</button>
<input id="inner-field" aria-label="Inner field">
<p id="out">idle</p>
</body></html>"#;

fn outer(inner_port: u16) -> String {
    format!(
        r#"<!doctype html><html><head><title>Outer</title></head><body>
<button id="outer-go">Outer go</button>
<iframe id="remote" title="Remote" src="http://localhost:{inner_port}/inner"></iframe>
<iframe srcdoc="<button>Local inner</button>"></iframe>
</body></html>"#
    )
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_cross_origin_iframe_is_read_and_acted_on_through_its_own_target() {
    let inner_port = serve(INNER.to_owned());
    let outer_port = serve(outer(inner_port));
    let chrome = launch("oopif");
    let mut connection = connect_to(&chrome);
    let page = open_tab(&mut connection, &format!("http://127.0.0.1:{outer_port}/"));
    wait_for_load(&mut connection, &page, deadline()).unwrap();

    let snapshot = snapshot_page(&mut connection, &page, deadline()).unwrap();

    assert!(
        snapshot.unreadable_frames.is_empty(),
        "{:?}",
        snapshot.unreadable_frames
    );
    let (_, outer_go) = find(&snapshot, "button", "Outer go");
    let (_, local_inner) = find(&snapshot, "button", "Local inner");
    let (inner_node, inner_go) = find(&snapshot, "button", "Inner go");
    let (_, inner_field) = find(&snapshot, "textfield", "Inner field");
    assert_eq!(outer_go.target_id, page.target_id);
    assert_eq!(local_inner.target_id, page.target_id);
    assert_ne!(inner_go.target_id, page.target_id);
    assert_eq!(
        inner_node.identity.native_id.as_ref().unwrap().value,
        "inner-go"
    );
    let preorder = snapshot.preorder();
    let owner_index = preorder
        .iter()
        .position(|(node, _)| {
            node.identity
                .native_id
                .as_ref()
                .is_some_and(|id| id.value == "remote")
        })
        .expect("the iframe element is in the tree");
    let inner_index = preorder
        .iter()
        .position(|(node, _)| node.identity.name.as_deref() == Some("Inner go"))
        .unwrap();
    assert!(inner_index > owner_index);

    let frame_session = attach_to_target(&mut connection, &inner_go.target_id, deadline()).unwrap();
    click(&mut connection, &frame_session, &inner_go, deadline()).unwrap();
    let filled = fill(
        &mut connection,
        &frame_session,
        &inner_field,
        "hello",
        deadline(),
    )
    .unwrap();
    assert!(filled.verified);
    let after = snapshot_page(&mut connection, &page, deadline()).unwrap();
    find(&after, "statictext", "pressed");
    let wrong_session = click(&mut connection, &page, &inner_go, deadline()).unwrap_err();
    assert_eq!(wrong_session.code, ErrorCode::InvalidArgs);
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_dom_hundreds_of_levels_deep_still_snapshots() {
    let depth = 300;
    let html = format!(
        "<!doctype html><title>Deep</title>{}<button id=\"deep-go\">Deep go</button>{}",
        "<div>".repeat(depth),
        "</div>".repeat(depth)
    );
    let chrome = launch("deep");
    let mut connection = connect_to(&chrome);
    let page = open_tab(&mut connection, &data_url(&html));

    let snapshot = snapshot_page(&mut connection, &page, deadline()).unwrap();

    let (node, token) = find(&snapshot, "button", "Deep go");
    assert_eq!(node.identity.native_id.as_ref().unwrap().value, "deep-go");
    read_element(&mut connection, &page, &token, deadline()).unwrap();
    assert_eq!(connection.dropped_event_count(), 0);
    assert!(
        connection
            .take_events()
            .iter()
            .all(|event| !event.method.starts_with("DOM.")),
        "the snapshot switched on DOM mutation events"
    );
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_ref_from_before_a_cross_site_navigation_is_stale() {
    let first = serve("<!doctype html><title>First</title><button>Old</button>".to_owned());
    let second = serve("<!doctype html><title>Second</title><button>New</button>".to_owned());
    let chrome = launch("swap");
    let mut connection = connect_to(&chrome);
    let page = open_tab(&mut connection, &format!("http://127.0.0.1:{first}/"));
    let snapshot = snapshot_page(&mut connection, &page, deadline()).unwrap();
    let (_, old) = find(&snapshot, "button", "Old");

    navigate(
        &mut connection,
        &page,
        &format!("http://localhost:{second}/"),
        deadline(),
    )
    .unwrap();
    let error = click(&mut connection, &page, &old, deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.details.unwrap()["kind"], "cdp_document_replaced");
    let fresh = snapshot_page(&mut connection, &page, deadline()).unwrap();
    let (_, new) = find(&fresh, "button", "New");
    click(&mut connection, &page, &new, deadline()).unwrap();
}
