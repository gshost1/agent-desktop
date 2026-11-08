use super::*;

#[test]
fn a_complete_walk_reports_only_what_it_visited() {
    let walk = MenuWalk {
        visited: 12,
        ..MenuWalk::default()
    };
    assert_eq!(
        serde_json::Value::Object(walk.report_fields()),
        serde_json::json!({ "visited": 12 })
    );
}

#[test]
fn a_partial_walk_names_what_it_skipped_and_its_slowest_read() {
    let walk = MenuWalk {
        unread_menus: vec!["Edit > AutoFill".into()],
        unread_after_budget: 31,
        visited: 400,
        slowest_read: Some((97, "Edit > AutoFill".into())),
        ..MenuWalk::default()
    };
    assert_eq!(
        serde_json::Value::Object(walk.report_fields()),
        serde_json::json!({
            "visited": 400,
            "unread_menus": ["Edit > AutoFill"],
            "unread_after_budget": 31,
            "slowest_read": { "ms": 97, "at": "Edit > AutoFill" }
        })
    );
}

#[test]
fn only_a_longer_read_replaces_the_slowest_one() {
    let mut walk = MenuWalk {
        slowest_read: Some((u128::MAX, "earlier".into())),
        ..MenuWalk::default()
    };
    walk.note_read(Instant::now(), || "later".into());
    assert_eq!(walk.slowest_read, Some((u128::MAX, "earlier".into())));
    let mut walk = MenuWalk::default();
    walk.note_read(Instant::now(), || "first".into());
    assert_eq!(
        walk.slowest_read.map(|(_, at)| at).as_deref(),
        Some("first")
    );
}
