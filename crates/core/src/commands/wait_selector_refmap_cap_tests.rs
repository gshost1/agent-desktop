use super::*;
use crate::commands::wait::refmap_cap_support::{
    LAST_BUTTON, OversizedTreeAdapter, oversized_window,
};
use crate::refs::refs_test_support::HomeGuard;

fn wait_for(needles: &[usize]) -> Value {
    let adapter = OversizedTreeAdapter {
        tree: oversized_window(needles),
    };
    execute(
        WaitSelectorInput {
            query_raw: "button:needle".into(),
            gone: false,
            app: Some("TestApp".into()),
            window_id: None,
            opts: crate::adapter::TreeOptions::default(),
            timeout_ms: 10_000,
        },
        &adapter,
        &CommandContext::default(),
    )
    .expect("the selector matches")
}

#[test]
fn a_selector_match_past_the_cut_says_it_was_cut() {
    let _guard = HomeGuard::new();
    let value = wait_for(&[LAST_BUTTON]);

    assert_eq!(value["matched_selector"], "button:needle");
    assert_eq!(value["refmap_capped"], true);
    assert_eq!(value["complete"], false);
    assert_eq!(value["match_cut"], true);
}

#[test]
fn a_selector_match_the_cut_kept_is_not_flagged() {
    let _guard = HomeGuard::new();
    let value = wait_for(&[0]);

    assert_eq!(value["refmap_capped"], true);
    assert!(value.get("match_cut").is_none());
}
