use super::NotificationKey;
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

fn digest(app: &str, title: &str, body: Option<&str>) -> u64 {
    NotificationKey::from(&info(app, title, body)).digest()
}

#[test]
fn the_digest_is_a_fixed_function_of_the_fields() {
    assert_eq!(digest("Mail", "a", None), digest("Mail", "a", None));
    assert_eq!(digest("Mail", "a", None), 0x3701_87c8_c549_66aa);
}

#[test]
fn every_field_and_its_boundaries_change_the_digest() {
    let base = digest("Mail", "ab", Some("c"));
    assert_ne!(base, digest("Chat", "ab", Some("c")));
    assert_ne!(base, digest("Mail", "ab", Some("d")));
    assert_ne!(base, digest("Mail", "a", Some("bc")));
    assert_ne!(digest("Mail", "a", None), digest("Mail", "a", Some("")));
}
