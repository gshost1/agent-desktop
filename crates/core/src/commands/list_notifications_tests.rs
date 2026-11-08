use super::keyed;
use crate::{NotificationIdentity, NotificationInfo, NotificationTarget};

fn info(index: usize, app: &str, body: &str) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: app.into(),
        title: "New message".into(),
        body: Some(body.into()),
        actions: vec![],
    }
}

#[test]
fn every_listed_entry_carries_a_key_and_twins_carry_different_keys() {
    let listed = [info(1, "Slack", "hi"), info(2, "Slack", "hi")];
    let entries = keyed(&listed).expect("keyed");
    let first = entries[0]["key"].as_str().expect("key");
    let second = entries[1]["key"].as_str().expect("key");
    assert_ne!(first, second);
    assert_eq!(entries[0]["index"], 1);
    assert_eq!(entries[0]["app_name"], "Slack");
}

#[test]
fn a_printed_key_names_its_entry_after_the_list_shifts() {
    let printed = keyed(&[info(1, "Slack", "from Ann")]).expect("keyed");
    let key = printed[0]["key"].as_str().expect("key");
    let target =
        NotificationTarget::new(None, Some(key), NotificationIdentity::default()).expect("target");
    let now = [info(1, "Slack", "from Bob"), info(2, "Slack", "from Ann")];
    assert_eq!(target.pick(&now).expect("found"), 1);
}
