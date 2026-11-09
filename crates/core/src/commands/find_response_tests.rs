use super::{format_match, format_response};
use crate::{
    LocatorQuery,
    commands::find::{FindArgs, FindFilterArgs, FindSelectionArgs},
    Rect,
    locator::live_locator::{
        LocatorMatchData, LocatorResolution, LocatorResolutionMeta, LocatorStats,
    },
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

fn match_data(ref_id: Option<&str>, role: &str, name: &str, path: &[&str]) -> LocatorMatchData {
    LocatorMatchData {
        ref_id: ref_id.map(String::from),
        role: role.into(),
        name: name.into(),
        value: None,
        states: Vec::new(),
        interactive: ref_id.is_some(),
        path: path.iter().map(|part| (*part).to_owned()).collect(),
    }
}

#[test]
fn matches_omit_absent_values_and_empty_lists_as_the_golden_file_shows() {
    let page = "webarea:Python (programming language) - Wikipedia";
    let root = match_data(None, "webarea", "Python (programming language) - Wikipedia", &[]);
    let block = match_data(
        None,
        "paragraph",
        "Python was conceived in the late 1980s",
        &[page, "main", "region:History"],
    );
    let mut link = match_data(Some("@s8f3k2p9:e2"), "link", "History", &[page, "navigation"]);
    link.states = vec!["selected".into()];
    let mut field = match_data(Some("@s8f3k2p9:e3"), "textfield", "", &[page, "main", "form"]);
    field.value = Some("Ada".into());
    let bounds = Rect {
        x: 10.0,
        y: 300.0,
        width: 600.0,
        height: 40.0,
    };

    let printed: Vec<serde_json::Value> = [(root, None), (block, Some(bounds)), (link, None), (field, None)]
        .into_iter()
        .map(|(data, bounds)| format_match(data, bounds).expect("a match serializes"))
        .collect();
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("../../../../tests/fixtures/find-matches.json"))
            .expect("golden parses");
    assert_eq!(serde_json::Value::from(printed), golden);
}
