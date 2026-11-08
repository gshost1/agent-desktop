//! Downloads a file from a local server in a real headless Chrome through
//! `WebChannelAdapter` with a download folder only each DevTools connection
//! points at, and checks that `wait --download` returns a path where the file
//! exists. Ignored by default; run with
//! `cargo test -p agent-desktop-web --test live_downloads -- --ignored`.

mod common;
mod download_bench;

use std::time::Duration;

use agent_desktop_core::{DownloadScope, OpenUrlRequest, SystemOps};
use download_bench::{Bench, CSV, assert_saved, deadline, serve};

#[test]
#[ignore = "launches headless Chrome"]
fn a_download_that_finished_before_a_later_command_waits_is_returned_where_it_exists() {
    let bench = Bench::new("dl-finished", DownloadScope::Connection, &[]);
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
    assert_eq!(
        info.url,
        Some(format!("http://127.0.0.1:{port}/report.csv"))
    );
    assert_saved(&bench, &info.path, "report.csv", &["report.csv"]);
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_download_still_receiving_when_a_later_command_waits_is_followed_on_disk() {
    let bench = Bench::new("dl-slow", DownloadScope::Connection, &[]);
    let port = serve(Duration::from_millis(1_500));
    bench.click_download(&bench.adapter(), port);

    let info = bench
        .adapter()
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    assert_eq!(info.bytes, CSV.len() as u64);
    assert_saved(&bench, &info.path, "report.csv", &["report.csv"]);
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_click_and_a_wait_in_one_invocation_follow_the_download_events() {
    let bench = Bench::new("dl-batch", DownloadScope::Connection, &[]);
    let port = serve(Duration::from_millis(300));
    let adapter = bench.adapter();
    bench.click_download(&adapter, port);

    let info = adapter
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    assert_saved(&bench, &info.path, "report.csv", &["report.csv"]);
}

#[test]
#[ignore = "launches headless Chrome"]
fn open_url_on_a_file_url_records_the_download_for_a_later_wait() {
    let bench = Bench::new("dl-open-url", DownloadScope::Connection, &[]);
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

    assert_saved(&bench, &info.path, "report.csv", &["report.csv"]);
}
