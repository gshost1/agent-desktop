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

fn stem(key: &str) -> &str {
    key.split_once("of").map_or(key, |parts| parts.0)
}

#[test]
fn the_oldest_twin_is_one_and_every_key_counts_its_twins() {
    let listed = [
        info("Mail", "x", None),
        info("Chat", "y", None),
        info("Mail", "x", None),
    ];
    let keys = keys(&listed);
    assert!(keys[0].ends_with("-2of2"), "{}", keys[0]);
    assert!(keys[1].ends_with("-1of1"), "{}", keys[1]);
    assert!(keys[2].ends_with("-1of2"), "{}", keys[2]);
    assert_eq!(
        keys[0].split_once('-').map(|p| p.0),
        keys[2].split_once('-').map(|p| p.0)
    );
    assert_ne!(keys[0], keys[2]);
}

#[test]
fn an_identical_newcomer_at_the_top_keeps_every_listed_key_naming_its_entry() {
    let before = [info("CI", "Build failed", None), info("Mail", "x", None)];
    let after = [
        info("CI", "Build failed", None),
        info("CI", "Build failed", None),
        info("Mail", "x", None),
    ];
    let listed = NotificationToken::assign(&before);
    let live = NotificationToken::assign(&after);
    for (was, now) in listed.iter().zip(&live[1..]) {
        assert!(was.names(now), "{was} should still name {now}");
        assert!(!was.lost_a_twin_by(now));
    }
    assert_eq!(stem(&keys(&before)[0]), stem(&keys(&after)[1]));
}

#[test]
fn a_twin_that_left_is_seen_by_the_newer_twins_key() {
    let listed = NotificationToken::assign(&[info("CI", "x", None), info("CI", "x", None)]);
    let live = NotificationToken::assign(&[info("CI", "x", None)]);
    let oldest = listed[1];
    assert!(
        oldest.names(&live[0]),
        "the newer twin took the oldest's number"
    );
    assert!(oldest.lost_a_twin_by(&live[0]));
}

#[test]
fn different_entries_whose_digests_collide_carry_one_key_instead_of_twin_numbers() {
    let identities: Vec<super::NotificationKey> =
        [info("Mail", "x", None), info("Chat", "y", None)]
            .iter()
            .map(super::NotificationKey::from)
            .collect();
    let tokens = NotificationToken::assign_by(&identities, |_| 7);
    assert_eq!(tokens[0], tokens[1]);
    assert_eq!(tokens[0].to_string(), "0000000000000007-1of1");
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
        "0123456789abcdef-1",
        "0123456789abcdef-0of1",
        "0123456789abcdef-1of0",
        "0123456789abcdef-2of1",
        "0123456789abcdef-of1",
        "0123456789abcdef-1of",
        "0123456789abcdef-",
        "0123456789abcdeg-1of1",
        "0123456789abcdef-+1of1",
    ] {
        assert!(NotificationToken::parse(bad).is_none(), "{bad}");
    }
}
