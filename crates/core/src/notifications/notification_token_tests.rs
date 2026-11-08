use super::NotificationToken;
use crate::NotificationInfo;

fn info(app: &str, title: &str, body: Option<&str>) -> NotificationInfo {
    NotificationInfo {
        index: 1,
        app_name: app.into(),
        title: title.into(),
        body: body.map(String::from),
        actions: vec![],
    }
}

fn keys(listed: &[NotificationInfo]) -> Vec<String> {
    NotificationToken::assign(listed)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn the_oldest_twin_is_one_and_other_entries_start_at_one() {
    let listed = [
        info("Mail", "x", None),
        info("Chat", "y", None),
        info("Mail", "x", None),
    ];
    let keys = keys(&listed);
    assert!(keys[0].ends_with("-2"));
    assert!(keys[1].ends_with("-1"));
    assert!(keys[2].ends_with("-1"));
    assert_eq!(
        keys[0].split_once('-').map(|p| p.0),
        keys[2].split_once('-').map(|p| p.0)
    );
    assert_ne!(keys[0], keys[2]);
}

#[test]
fn an_identical_newcomer_at_the_top_does_not_change_the_listed_keys() {
    let before = [info("CI", "Build failed", None), info("Mail", "x", None)];
    let after = [
        info("CI", "Build failed", None),
        info("CI", "Build failed", None),
        info("Mail", "x", None),
    ];
    assert_eq!(keys(&before)[..], keys(&after)[1..]);
}

#[test]
fn a_printed_key_parses_back_and_malformed_keys_do_not() {
    let listed = [info("Mail", "x", Some("b"))];
    let printed = keys(&listed).remove(0);
    let parsed = NotificationToken::parse(&printed).expect("round trip");
    assert_eq!(parsed.to_string(), printed);
    for bad in [
        "",
        "3",
        "abc-1",
        "0123456789abcdef-0",
        "0123456789abcdef-",
        "0123456789abcdeg-1",
        "0123456789abcdef-+1",
    ] {
        assert!(NotificationToken::parse(bad).is_none(), "{bad}");
    }
}
