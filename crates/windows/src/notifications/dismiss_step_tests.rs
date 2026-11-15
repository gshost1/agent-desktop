use agent_desktop_core::{NotificationIdentity, NotificationInfo, NotificationTarget};

use super::{DismissStep, after_dismiss, is_sole_target};

fn entry(index: usize, app: &str, title: &str) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: app.into(),
        title: title.into(),
        body: Some("same body".into()),
        actions: Vec::new(),
    }
}

fn twins() -> Vec<NotificationInfo> {
    vec![entry(1, "App A", "Ping"), entry(2, "App A", "Ping")]
}

fn oldest_twin(before: &[NotificationInfo]) -> NotificationInfo {
    let key = agent_desktop_core::notification_keys(before)[1].clone();
    let target =
        NotificationTarget::new(None, Some(&key), NotificationIdentity::default()).expect("target");
    let at = target
        .pick(before.iter())
        .expect("the oldest twin is picked");
    before[at].clone()
}

#[test]
fn dismissing_one_of_two_identical_twins_is_verified_by_the_twin_count_dropping() {
    let before = twins();
    let target = oldest_twin(&before);
    let current = vec![entry(1, "App A", "Ping")];

    assert_eq!(
        after_dismiss(&target, &before, &current),
        DismissStep::Verified,
        "the keyed twin left and its untouched twin remains, so the dismiss landed"
    );
}

#[test]
fn a_lone_survivor_of_two_listed_twins_never_triggers_the_clear_all() {
    let before = twins();
    let target = oldest_twin(&before);
    let current = vec![entry(1, "App A", "Ping")];

    assert_ne!(
        after_dismiss(&target, &before, &current),
        DismissStep::SubstituteClearAll,
        "the survivor is indistinguishable from the target, so the clear-all could delete the untouched twin"
    );
}

#[test]
fn an_ignored_dismiss_of_one_twin_survives_without_touching_the_other() {
    let before = twins();
    let target = oldest_twin(&before);

    assert_eq!(
        after_dismiss(&target, &before, &before),
        DismissStep::Survived
    );
}

#[test]
fn an_ignored_dismiss_of_a_sole_entry_substitutes_the_clear_all() {
    let before = vec![entry(1, "App A", "Ping")];

    assert_eq!(
        after_dismiss(&before[0], &before, &before),
        DismissStep::SubstituteClearAll
    );
}

#[test]
fn a_sole_entry_dismissed_is_verified() {
    let before = vec![entry(1, "App A", "Ping")];

    assert_eq!(
        after_dismiss(&before[0], &before, &[]),
        DismissStep::Verified
    );
}

#[test]
fn the_clear_all_substitute_fires_only_on_exactly_one_matching_entry() {
    let target = entry(1, "Toaster", "Build finished");
    let foreign = entry(1, "Toaster", "Build finished (not the staged entry)");

    assert!(is_sole_target(&target, std::slice::from_ref(&target)));
    assert!(!is_sole_target(&target, std::slice::from_ref(&foreign)));
    assert!(!is_sole_target(&target, &[]));
    assert!(!is_sole_target(&target, &[target.clone(), foreign]));
}
