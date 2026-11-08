use super::refmap_cap_support::{LAST_BUTTON, OversizedTreeAdapter, oversized_window};
use super::*;
use crate::refs::refs_store::RefStore;
use crate::refs::refs_test_support::HomeGuard;

fn text_wait(text: &str, count: Option<usize>) -> WaitArgs {
    use super::test_support::wait_args;
    WaitArgs {
        mode: WaitModeArgs {
            text: Some(text.into()),
            ..wait_args().mode
        },
        predicate: WaitPredicateArgs {
            count,
            ..wait_args().predicate
        },
        timeout_ms: 10_000,
        app: Some("TestApp".into()),
    }
}

fn run(needles: &[usize], count: Option<usize>) -> Value {
    let adapter = OversizedTreeAdapter {
        tree: oversized_window(needles),
    };
    execute(
        text_wait("needle", count),
        &adapter,
        &CommandContext::default(),
    )
    .expect("the text is on the page")
}

#[test]
fn a_text_match_past_the_cut_says_it_was_cut() {
    let _guard = HomeGuard::new();
    let value = run(&[LAST_BUTTON], None);

    assert_eq!(value["found"], true);
    assert!(value["ref"].is_null());
    assert_eq!(value["complete"], false);
    assert_eq!(value["refmap_capped"], true);
    assert_eq!(value["match_cut"], true);
}

#[test]
fn a_counted_text_match_reports_what_the_saved_snapshot_holds() {
    let _guard = HomeGuard::new();
    let value = run(&[0, LAST_BUTTON], Some(2));

    assert_eq!(value["count"], 2, "the complete read found both");
    assert_eq!(value["stored_count"], 1);
    assert_eq!(value["complete"], false);
    assert_eq!(value["refmap_capped"], true);
    assert!(value.get("match_cut").is_none());
    let ref_id = value["ref"].as_str().expect("the first match is stored");
    RefStore::new()
        .unwrap()
        .load_ref(ref_id)
        .expect("the reported ref is in the saved snapshot");
}

#[test]
fn an_uncut_text_match_carries_no_cut_fields() {
    let _guard = HomeGuard::new();
    let mut tree = oversized_window(&[3]);
    tree.children.truncate(10);
    let value = execute(
        text_wait("needle", Some(1)),
        &OversizedTreeAdapter { tree },
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["count"], 1);
    assert!(value["ref"].is_string());
    for field in ["complete", "refmap_capped", "match_cut", "stored_count"] {
        assert!(value.get(field).is_none(), "{field} is only for a cut");
    }
}
