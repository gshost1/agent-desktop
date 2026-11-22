use super::test_support::LiveFindAdapter;
use super::*;
use crate::refs::refs_test_support::HomeGuard;

fn find_in_unreadable_window(name: &str, selection: FindSelectionArgs) -> Result<Value, AppError> {
    execute(
        FindArgs {
            scope: FindScopeArgs {
                app: None,
                window_id: Some("w-1".into()),
                root: None,
                surface: crate::SnapshotSurface::Window,
                scroll: None,
            },
            filter: FindFilterArgs {
                name: Some(name.into()),
                role: None,
                description: None,
                native_id: None,
                value: None,
                text: None,
                exact: false,
            },
            states: Vec::new(),
            timeout_ms: None,
            selection,
        },
        &LiveFindAdapter::incomplete(),
        &CommandContext::default(),
    )
}

fn listing() -> FindSelectionArgs {
    FindSelectionArgs {
        count: false,
        first: false,
        last: false,
        nth: None,
        limit: None,
        allow_partial: false,
    }
}

fn unreadable_details(error: AppError) -> serde_json::Value {
    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    let suggestion = error.suggestion.clone().unwrap_or_default();
    assert!(
        !suggestion.contains("Raise --timeout-ms"),
        "an unreadable part is not a budget problem: {suggestion}"
    );
    error.details.expect("details")
}

#[test]
fn a_match_seen_beside_an_unreadable_part_is_listed_and_marked_incomplete() {
    let _guard = HomeGuard::new();
    let response = find_in_unreadable_window("OnlyInWindowOne", listing())
        .expect("an observed match is an answer, not a timeout");

    assert_eq!(response["total_matches"], 1);
    assert_eq!(response["matches"][0]["name"], "OnlyInWindowOne");
    assert_eq!(response["complete"], false);
    assert_eq!(response["truncated"], true);
}

#[test]
fn no_match_beside_an_unreadable_part_refuses_without_claiming_absence_or_a_timeout() {
    let _guard = HomeGuard::new();
    let error = find_in_unreadable_window("missing", listing())
        .expect_err("the unread part could hold the match");

    assert_eq!(error.code(), "ELEMENT_NOT_FOUND");
    let details = unreadable_details(error);
    assert_eq!(details["kind"], "locator_unreadable_part");
    assert_eq!(details["complete"], false);
    assert_eq!(details["observed_matches"], 0);
}

#[test]
fn a_count_or_positional_pick_beside_an_unreadable_part_stays_refused() {
    let _guard = HomeGuard::new();
    for selection in [
        FindSelectionArgs {
            count: true,
            ..listing()
        },
        FindSelectionArgs {
            last: true,
            ..listing()
        },
        FindSelectionArgs {
            nth: Some(0),
            ..listing()
        },
    ] {
        let error = find_in_unreadable_window("OnlyInWindowOne", selection)
            .expect_err("a match in the unread part would change the answer");
        assert_eq!(error.code(), "TIMEOUT");
        let details = unreadable_details(error);
        assert_eq!(details["kind"], "locator_unreadable_part");
        assert_eq!(details["observed_matches"], 1);
    }
}

#[test]
fn allow_partial_keeps_its_own_contract_beside_an_unreadable_part() {
    let _guard = HomeGuard::new();
    let response = find_in_unreadable_window(
        "missing",
        FindSelectionArgs {
            allow_partial: true,
            ..listing()
        },
    )
    .expect("allow-partial answers for the readable part");
    assert_eq!(response["total_matches"], 0);
    assert_eq!(response["complete"], false);
}
