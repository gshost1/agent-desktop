use super::SnapshotChannel;

#[test]
fn a_cdp_channel_writes_its_page_and_omits_what_it_lacks() {
    let mut body = serde_json::json!({ "app": "Browser" });
    SnapshotChannel {
        channel: "cdp".into(),
        url: Some("https://example.com/".into()),
        title: Some("Example".into()),
        suggestion: None,
    }
    .write_into(&mut body);
    assert_eq!(body["channel"], "cdp");
    assert_eq!(body["url"], "https://example.com/");
    assert_eq!(body["title"], "Example");
    assert!(body.get("suggestion").is_none());
}

#[test]
fn an_ax_fallback_carries_its_suggestion_and_no_page() {
    let mut body = serde_json::json!({});
    SnapshotChannel {
        channel: "ax".into(),
        suggestion: Some("launch the browser with --cdp".into()),
        ..SnapshotChannel::default()
    }
    .write_into(&mut body);
    assert_eq!(body["channel"], "ax");
    assert_eq!(body["suggestion"], "launch the browser with --cdp");
    assert!(body.get("url").is_none());
    assert!(body.get("title").is_none());
}

#[test]
fn a_snapshot_without_a_channel_has_no_channel_fields() {
    let result = crate::snapshot::SnapshotResult {
        tree: crate::adapter::minimal_accessibility_node("window", "Fixture"),
        refmap: crate::RefMap::new(),
        owner: crate::SnapshotOwner::Application(crate::AppInfo {
            name: "Fixture".into(),
            pid: crate::ProcessId::new(1),
            process_instance: None,
            bundle_id: None,
            presentation: None,
        }),
        snapshot_id: None,
        complete: true,
        nodes_observed: 1,
        notes: crate::SnapshotNotes::default(),
    };
    let body = crate::commands::snapshot::format_snapshot_fields(&result, None, None).unwrap();
    for key in ["channel", "url", "title", "suggestion"] {
        assert!(body.get(key).is_none(), "{key}");
    }
}
