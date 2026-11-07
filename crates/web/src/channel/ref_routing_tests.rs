use std::borrow::Cow;

use agent_desktop_core::{ProcessId, ProcessIdentity, SnapshotSurface, WindowInfo, WindowState};

use super::{platform_entry, web_token, window_process};
use crate::channel::test_support::web_entry;

#[test]
fn only_devtools_tokens_route_to_the_web_channel() {
    let web = web_entry("cdp:T:4", "button", "Go");
    assert_eq!(web_token(&web), Some("cdp:T:4"));
    let native = web_entry("axtok1:abc", "button", "Go");
    assert_eq!(web_token(&native), None);
}

#[test]
fn a_web_surface_ref_reaches_the_platform_as_a_window_ref() {
    let entry = web_entry("axtok1:abc", "button", "Go");
    let translated = platform_entry(&entry);
    assert!(matches!(translated, Cow::Owned(_)));
    assert_eq!(translated.source.source_surface, SnapshotSurface::Window);
    let mut window_ref = entry.clone();
    window_ref.source.source_surface = SnapshotSurface::Window;
    assert!(matches!(platform_entry(&window_ref), Cow::Borrowed(_)));
}

#[test]
fn a_window_without_a_process_instance_has_no_process() {
    let mut window = WindowInfo {
        id: "w".into(),
        title: "t".into(),
        app: "Browser".into(),
        pid: ProcessId::new(3),
        process_instance: Some("i".into()),
        bounds: None,
        state: WindowState::default(),
    };
    assert_eq!(
        window_process(&window),
        Some(ProcessIdentity::new(3u32, "i"))
    );
    window.process_instance = None;
    assert_eq!(window_process(&window), None);
}
