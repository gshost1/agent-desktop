//! A web snapshot taken right after a click that navigates reads the page
//! the click led to, even when that page takes longer to answer than the
//! click takes to return, so an agent needs no sleep between the two.
//! Ignored by default; run with
//! `cargo test -p agent-desktop-web --test live_navigation_settle -- --ignored`.

mod common;
mod download_bench;

use agent_desktop_core::DownloadScope;
use download_bench::{Bench, SLOW_PAGE, serve};
use std::time::{Duration, Instant};

fn shows(node: &agent_desktop_core::AccessibilityNode, name: &str) -> bool {
    node.identity.name.as_deref() == Some(name)
        || node.children.iter().any(|child| shows(child, name))
}

fn snapshot_after_click_reads_the_next_page(control: &str) {
    for scope in [DownloadScope::Profile, DownloadScope::Connection] {
        let bench = Bench::new("navigation-snapshot", scope, &[]);
        let port = serve(Duration::ZERO);
        let adapter = bench.adapter();

        let clicked = bench.click_named(&adapter, port, control);
        let started = Instant::now();
        let page = bench.snapshot(&adapter);
        let snapshot = started.elapsed();

        eprintln!("{scope:?} {control}: click {clicked:?}, snapshot {snapshot:?}");
        assert!(
            shows(&page, "Landed here"),
            "{scope:?} {control}: the snapshot after the click still shows the old page"
        );
        assert!(
            clicked + snapshot >= SLOW_PAGE,
            "{scope:?} {control}: {clicked:?} + {snapshot:?}"
        );
    }
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_snapshot_after_a_link_click_reads_the_page_the_link_led_to() {
    snapshot_after_click_reads_the_next_page("Next page");
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_snapshot_after_a_click_whose_script_navigates_reads_the_next_page() {
    snapshot_after_click_reads_the_next_page("Go by script");
}
