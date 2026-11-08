//! Live headless-Chrome checks that a click on a link lands on the link and
//! navigates: an inline link the page must scroll to, a link that wraps over
//! two lines so the centre of its bounding box lands between them, and a
//! centred page laid out like example.com. Ignored by
//! default; run with `cargo test -p agent-desktop-web --test live_clicks -- --ignored`.

mod common;

use std::time::{Duration, Instant};

use agent_desktop_core::ErrorCode;
use agent_desktop_web::{
    CdpConnection, ElementToken, PageSession, click, snapshot_page, wait_for_load,
};
use common::{connect_to, deadline, find, launch, open_tab, serve};
use serde_json::{Value, json};

const BELOW_THE_FOLD: &str = r#"<!doctype html><html><head><title>Fold</title></head><body>
<div style="height:2400px">Spacer</div>
<a href="/landed?below">Learn more</a>
</body></html>"#;

const WRAPPED: &str = r#"<!doctype html><html><head><title>Wrapped</title></head><body>
<p style="width:20ch;font:16px/3 monospace;margin:0">xxxxxxxxxxxxxxx <a href="/landed?wrapped">link text</a> yyyyyyyyyyyyyyy</p>
</body></html>"#;

const WRAPPED_BELOW: &str = r#"<!doctype html><html><head><title>Wrapped below</title></head><body>
<div style="height:2400px">Spacer</div>
<p style="width:20ch;font:16px/3 monospace;margin:0">xxxxxxxxxxxxxxx <a href="/landed?wrapped-below">link text</a> yyyyyyyyyyyyyyy</p>
</body></html>"#;

const COVERED: &str = r#"<!doctype html><html><head><title>Covered</title></head><body>
<a href="/landed?covered">Hidden link</a>
<div style="position:fixed;inset:0;background:white">Overlay</div>
</body></html>"#;

const CENTRED: &str = r#"<!doctype html><html lang=en><head><meta charset=utf-8>
<meta name=viewport content="width=device-width,initial-scale=1"><title>Centred</title>
<style>body{font:16px/1.6 system-ui,sans-serif;max-width:26em;margin:auto;padding:25vh 2em 2em;text-align:center}p+p{font-size:.875em}</style>
</head><body><p>This domain is for use in documentation examples without needing permission.</p>
<p>Second paragraph that takes a few lines of text so the page grows taller than a small viewport would show at once.</p>
<p>Third paragraph that takes a few lines of text so the page grows taller than a small viewport would show at once.</p>
<p>Fourth paragraph that takes a few lines of text so the page grows taller than a small viewport would show at once.</p>
<p>Fifth paragraph that takes a few lines of text so the page grows taller than a small viewport would show at once.</p>
<p>Sixth paragraph that takes a few lines of text so the page grows taller than a small viewport would show at once.</p>
<a href=/landed?centred>Learn more</a></body></html>"#;

struct Clicked {
    _chrome: common::HeadlessChrome,
    connection: CdpConnection,
    page: PageSession,
    link: ElementToken,
}

fn href(connection: &mut CdpConnection, page: &PageSession) -> String {
    connection
        .call(
            "Runtime.evaluate",
            json!({ "expression": "location.href", "returnByValue": true }),
            Some(&page.session_id),
            deadline(),
        )
        .ok()
        .and_then(|reply| {
            reply
                .pointer("/result/value")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

fn link_on(html: &str, viewport_height: u32, label: &str) -> Clicked {
    let port = serve(html.to_owned());
    let chrome = launch(label);
    let mut connection = connect_to(&chrome);
    let page = open_tab(&mut connection, &format!("http://127.0.0.1:{port}/"));
    connection
        .call(
            "Emulation.setDeviceMetricsOverride",
            json!({ "width": 800, "height": viewport_height, "deviceScaleFactor": 2, "mobile": false }),
            Some(&page.session_id),
            deadline(),
        )
        .unwrap();
    wait_for_load(&mut connection, &page, deadline()).unwrap();
    let snapshot = snapshot_page(&mut connection, &page, deadline()).unwrap();
    let name = snapshot
        .preorder()
        .into_iter()
        .find(|(node, _)| node.role == "link")
        .and_then(|(node, _)| node.identity.name.clone())
        .expect("the page has a link");
    let (_, link) = find(&snapshot, "link", &name);
    Clicked {
        _chrome: chrome,
        connection,
        page,
        link,
    }
}

fn clicks_and_lands(html: &str, viewport_height: u32, landed: &str) {
    let Clicked {
        _chrome,
        mut connection,
        page,
        link,
    } = link_on(html, viewport_height, landed);

    click(&mut connection, &page, &link, deadline())
        .unwrap_or_else(|error| panic!("{landed}: {error:?}"));

    let started = Instant::now();
    let mut url = href(&mut connection, &page);
    while !url.ends_with(&format!("/landed?{landed}")) {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{landed}: still at {url}"
        );
        std::thread::sleep(Duration::from_millis(50));
        url = href(&mut connection, &page);
    }
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_link_the_page_scrolls_to_is_clicked_where_it_now_shows() {
    clicks_and_lands(BELOW_THE_FOLD, 600, "below");
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_link_that_wraps_over_two_lines_is_clicked_on_one_of_its_lines() {
    clicks_and_lands(WRAPPED, 600, "wrapped");
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_wrapped_link_below_the_fold_is_clicked_after_the_scroll() {
    clicks_and_lands(WRAPPED_BELOW, 600, "wrapped-below");
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_centred_page_link_after_the_text_is_clicked() {
    clicks_and_lands(CENTRED, 500, "centred");
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_link_an_overlay_covers_is_still_refused() {
    let Clicked {
        _chrome,
        mut connection,
        page,
        link,
    } = link_on(COVERED, 600, "covered");

    let error = click(&mut connection, &page, &link, deadline()).unwrap_err();

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.details.unwrap()["kind"], "cdp_click_intercepted");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!href(&mut connection, &page).contains("landed"));
}
