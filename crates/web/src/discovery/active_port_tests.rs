use std::path::PathBuf;

use super::{parse_active_port, read_active_port};

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent-desktop-web-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn parses_port_and_path() {
    let parsed = parse_active_port("9222\n/devtools/browser/6f1c\n").unwrap();
    assert_eq!(parsed.port, 9222);
    assert_eq!(parsed.websocket_path, "/devtools/browser/6f1c");
}

#[test]
fn rejects_malformed_contents() {
    for text in [
        "",
        "abc\n/devtools/browser/x",
        "0\n/x",
        "9222\n",
        "9222\nrelative",
        "9222\n/",
    ] {
        assert!(parse_active_port(text).is_err(), "{text:?}");
    }
}

#[test]
fn reading_leaves_the_file_untouched() {
    let dir = scratch_dir("active-port");
    let file = dir.join("DevToolsActivePort");
    std::fs::write(&file, "41234\n/devtools/browser/abc\n").unwrap();
    let before = std::fs::metadata(&file).unwrap().modified().unwrap();

    let parsed = read_active_port(&dir).unwrap();

    assert_eq!(parsed.port, 41234);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "41234\n/devtools/browser/abc\n"
    );
    assert_eq!(
        std::fs::metadata(&file).unwrap().modified().unwrap(),
        before
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_missing_file_is_a_structured_error() {
    let dir = scratch_dir("active-port-missing");
    let error = read_active_port(&dir).unwrap_err();
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "cdp_active_port_missing"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
