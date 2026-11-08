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
fn twins_get_increasing_occurrences_and_other_entries_start_at_one() {
    let listed = [
        info("Mail", "x", None),
        info("Chat", "y", None),
        info("Mail", "x", None),
    ];
    let keys = keys(&listed);
    assert!(keys[0].ends_with("-1"));
    assert!(keys[1].ends_with("-1"));
    assert!(keys[2].ends_with("-2"));
    assert_eq!(
        keys[0].split_once('-').map(|p| p.0),
        keys[2].split_once('-').map(|p| p.0)
    );
    assert_ne!(keys[0], keys[2]);
}

#[test]
fn a_filtered_list_assigns_the_same_keys_as_the_full_list() {
    let full = [
        info("Chat", "y", None),
        info("Mail", "x", None),
        info("Mail", "x", None),
    ];
    let mail_only = [full[1].clone(), full[2].clone()];
    assert_eq!(keys(&full)[1..], keys(&mail_only)[..]);
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
