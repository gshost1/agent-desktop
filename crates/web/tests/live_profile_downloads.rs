//! Downloads from a local server in a real headless Chrome whose profile
//! `prepare_cdp_downloads` pointed at a download folder before launch, as
//! `launch --cdp --download-dir` does. The browser then saves into the
//! folder under real names even after the clicking command's connection has
//! closed, so a click holds no connection open for the download to begin.
//! Ignored by default; run with
//! `cargo test -p agent-desktop-web --test live_profile_downloads -- --ignored`.

mod common;
mod download_bench;

use std::time::Duration;

use agent_desktop_core::{DownloadScope, ErrorCode, SystemOps};
use download_bench::{Bench, assert_saved, deadline, serve};

#[test]
#[ignore = "launches headless Chrome"]
fn a_click_in_one_command_is_saved_under_its_name_and_returned_by_a_later_wait() {
    let bench = Bench::new("profile-later", DownloadScope::Profile, &["notes.txt"]);
    let port = serve(Duration::ZERO);

    let click = bench.click_download(&bench.adapter(), port);
    let first = bench
        .adapter()
        .wait_for_download(bench.process(), deadline())
        .unwrap();
    bench.click_download(&bench.adapter(), port);
    let second = bench
        .adapter()
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    eprintln!("profile-scope click took {click:?}");
    assert!(click < Duration::from_millis(1_000), "{click:?}");
    assert_eq!(first.state, "completed");
    assert_saved(
        &bench,
        &first.path,
        "report.csv",
        &["notes.txt", "report (1).csv", "report.csv"],
    );
    assert_saved(
        &bench,
        &second.path,
        "report (1).csv",
        &["notes.txt", "report (1).csv", "report.csv"],
    );
    let none_left = bench
        .adapter()
        .wait_for_download(
            bench.process(),
            agent_desktop_core::Deadline::after(800).unwrap(),
        )
        .unwrap_err();
    assert_eq!(none_left.code, ErrorCode::Timeout);
    assert_eq!(
        none_left.details.unwrap()["kind"],
        "cdp_download_not_started"
    );
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_download_still_receiving_is_followed_in_the_folder_until_it_finishes() {
    let bench = Bench::new("profile-slow", DownloadScope::Profile, &[]);
    let port = serve(Duration::from_millis(1_500));
    bench.click_download(&bench.adapter(), port);

    let info = bench
        .adapter()
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    assert_saved(&bench, &info.path, "report.csv", &["report.csv"]);
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_click_and_a_wait_in_one_invocation_report_where_the_file_came_from() {
    let bench = Bench::new("profile-batch", DownloadScope::Profile, &[]);
    let port = serve(Duration::from_millis(300));
    let adapter = bench.adapter();
    bench.click_download(&adapter, port);

    let info = adapter
        .wait_for_download(bench.process(), deadline())
        .unwrap();

    assert_eq!(
        info.url,
        Some(format!("http://127.0.0.1:{port}/report.csv"))
    );
    assert_saved(&bench, &info.path, "report.csv", &["report.csv"]);
}
