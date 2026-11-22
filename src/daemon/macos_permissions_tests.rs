use super::permits_forwarding;

fn permits(args: &[&str], accessibility: bool, screen_recording: bool) -> bool {
    let argv = std::iter::once("agent-desktop")
        .chain(args.iter().copied())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    permits_forwarding(&argv, false, || accessibility, || screen_recording)
}

#[test]
fn input_stays_in_the_caller_without_accessibility() {
    let click = ["--daemon", "click", "@s1234abcd:e1"];
    assert!(!permits(&click, false, true));
    assert!(permits(&click, true, false));
    assert!(permits(&["version"], false, false));
}

#[test]
fn screenshots_stay_in_the_caller_without_screen_recording() {
    let screen = ["screenshot", "--screen", "0", "shot.png"];
    let window = ["screenshot", "--app", "Finder", "shot.png"];
    assert!(!permits(&screen, true, false));
    assert!(permits(&screen, false, true));
    assert!(!permits(&window, false, true));
    assert!(permits(&window, true, true));
}

#[test]
fn batch_cannot_borrow_either_grant_from_the_daemon() {
    let args = [
        "batch",
        r#"[{"command":"version"},{"command":"click","args":{"ref_id":"@s1234abcd:e1"}},{"command":"screenshot","args":{"screen":0,"output_path":"shot.png"}}]"#,
    ];
    assert!(!permits(&args, false, true));
    assert!(!permits(&args, true, false));
    assert!(permits(&args, true, true));
}

#[test]
fn potential_implicit_screen_capture_stays_in_the_caller_without_the_grant() {
    let debug = [
        "--debug",
        "--screenshot",
        "debug.html",
        "click",
        "@s1234abcd:e1",
    ];
    let session = ["--session", "example", "click", "@s1234abcd:e1"];
    for args in [debug.as_slice(), session.as_slice()] {
        assert!(!permits(args, true, false));
        assert!(permits(args, true, true));
    }
}

#[test]
fn window_inventory_and_permission_requests_respect_the_caller() {
    let windows = ["list-windows"];
    assert!(!permits(&windows, true, false));
    assert!(!permits(&windows, false, true));
    assert!(permits(&windows, true, true));
    assert!(!permits(&["permissions", "--request"], true, true));
}

#[test]
fn grant_reports_always_describe_the_caller() {
    for args in [
        &["status"][..],
        &["permissions"][..],
        &["permissions", "--guide"][..],
    ] {
        assert!(!permits(args, true, true));
        assert!(!permits(args, false, false));
    }
}
