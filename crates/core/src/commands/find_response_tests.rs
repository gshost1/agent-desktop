use super::format_response;
use crate::{
    LocatorQuery,
    commands::find::{FindArgs, FindFilterArgs, FindSelectionArgs},
    locator::live_locator::{LocatorResolution, LocatorResolutionMeta, LocatorStats},
};

fn args(allow_partial: bool, count: bool) -> FindArgs {
    FindArgs {
        app: None,
        window_id: None,
        root: None,
        surface: crate::SnapshotSurface::Window,
        filter: FindFilterArgs {
            role: None,
            name: None,
            description: None,
            native_id: None,
            value: None,
            text: None,
            exact: false,
        },
        states: Vec::new(),
        timeout_ms: None,
        selection: FindSelectionArgs {
            count,
            first: false,
            last: false,
            nth: None,
            limit: None,
            allow_partial,
        },
    }
}

fn resolution(selection_complete: bool) -> LocatorResolution {
    LocatorResolution {
        matches: Vec::new(),
        refmap: None,
        stats: LocatorStats::default(),
        meta: LocatorResolutionMeta {
            total_matches: 2,
            complete: selection_complete,
            selection_complete,
            truncated: false,
            roles_present: Vec::new(),
        },
    }
}

#[test]
fn a_partial_answer_says_it_is_incomplete_and_truncated() {
    let query = LocatorQuery::default();
    for count in [false, true] {
        let response =
            format_response(&args(true, count), &query, resolution(false), None).unwrap();
        assert_eq!(response["complete"], false, "count={count}");
        assert_eq!(response["truncated"], true, "count={count}");
    }

    let complete = format_response(&args(true, false), &query, resolution(true), None).unwrap();
    assert_eq!(complete["complete"], true);
    assert_eq!(complete["truncated"], false);
}

#[test]
fn the_default_contract_carries_no_completeness_field() {
    let response = format_response(
        &args(false, false),
        &LocatorQuery::default(),
        resolution(true),
        None,
    )
    .unwrap();
    assert!(response.get("complete").is_none());
    assert_eq!(response["total_matches"], 2);
}
