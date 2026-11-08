use super::NotificationBaseline;
use crate::NotificationInfo;

fn entry(index: usize, title: &str, actions: &[&str]) -> NotificationInfo {
    NotificationInfo {
        index,
        app_name: "Mail".into(),
        title: title.into(),
        body: None,
        actions: actions.iter().map(|action| (*action).into()).collect(),
    }
}

#[test]
fn a_reindexed_existing_entry_is_not_new() {
    let mut baseline = NotificationBaseline::from_list(&[entry(0, "old", &[])]);

    assert!(baseline.observe(&[entry(4, "old", &[])]).is_none());
}

#[test]
fn a_duplicate_beyond_the_recorded_count_is_new() {
    let mut baseline = NotificationBaseline::from_list(&[entry(0, "same", &[])]);

    let current = [entry(4, "same", &[]), entry(5, "same", &[])];
    let found = baseline.observe(&current).map(|info| info.index);

    assert_eq!(found, Some(5));
}

#[test]
fn an_entry_gone_from_two_lists_is_new_when_it_returns() {
    let mut baseline = NotificationBaseline::from_list(&[entry(0, "A", &[])]);

    assert!(baseline.observe(&[]).is_none());
    assert!(baseline.observe(&[]).is_none());
    let current = [entry(0, "A", &[])];

    assert_eq!(baseline.observe(&current).map(|info| info.index), Some(0));
}

#[test]
fn one_short_read_does_not_lower_the_record() {
    let full = [entry(0, "A", &[])];
    let mut baseline = NotificationBaseline::from_list(&full);

    assert!(baseline.observe(&[]).is_none());

    assert!(baseline.observe(&full).is_none());
}

#[test]
fn a_short_read_then_a_different_short_read_does_not_lower_the_record() {
    let three = [entry(0, "A", &[]), entry(1, "A", &[]), entry(2, "A", &[])];
    let mut baseline = NotificationBaseline::from_list(&three);

    assert!(baseline.observe(&three[..1]).is_none());
    assert!(baseline.observe(&three[..2]).is_none());

    assert!(baseline.observe(&three).is_none());
}

#[test]
fn one_of_two_twins_leaving_and_returning_is_new() {
    let twins = [entry(0, "A", &[]), entry(1, "A", &[])];
    let mut baseline = NotificationBaseline::from_list(&twins);

    assert!(baseline.observe(&twins[..1]).is_none());
    assert!(baseline.observe(&twins[..1]).is_none());

    assert_eq!(baseline.observe(&twins).map(|info| info.index), Some(1));
}

#[test]
fn changed_actions_alone_do_not_make_an_entry_new() {
    let mut baseline = NotificationBaseline::from_list(&[entry(0, "Invite", &[])]);

    assert!(
        baseline
            .observe(&[entry(0, "Invite", &["Accept", "Decline"])])
            .is_none()
    );
}

#[test]
fn a_matching_list_does_not_replace_the_record() {
    let mut baseline = NotificationBaseline::from_list(&[]);
    let current = [entry(0, "new", &[])];

    assert!(baseline.observe(&current).is_some());

    assert!(baseline.observe(&current).is_some());
}
