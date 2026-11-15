use super::test_support::LiveFindAdapter;
use super::*;
use crate::refs::refs_test_support::HomeGuard;

const SUBSTRING_MATCHES: usize = 60;

fn longer_names_then_the_exact_one() -> Vec<String> {
    (0..SUBSTRING_MATCHES)
        .map(|index| format!("Save As {index}"))
        .chain(std::iter::once("Save".to_string()))
        .collect()
}

fn find_by_name(name: &str, limit: Option<usize>) -> FindArgs {
    FindArgs {
        scope: FindScopeArgs {
            app: None,
            window_id: Some("w-1".into()),
            root: None,
            surface: crate::SnapshotSurface::Window,
        },
        filter: FindFilterArgs {
            role: None,
            name: Some(name.into()),
            description: None,
            native_id: None,
            value: None,
            text: None,
            exact: false,
        },
        states: Vec::new(),
        timeout_ms: None,
        selection: FindSelectionArgs {
            count: false,
            first: false,
            last: false,
            nth: None,
            limit,
            allow_partial: false,
        },
    }
}

fn names(response: &Value) -> Vec<String> {
    response["matches"]
        .as_array()
        .map(|matches| {
            matches
                .iter()
                .filter_map(|found| found["name"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn an_unlimited_find_returns_the_exact_name_before_longer_names_found_earlier() {
    let _guard = HomeGuard::new();
    let adapter = LiveFindAdapter::with_buttons(&longer_names_then_the_exact_one());

    let response = execute(
        find_by_name("Save", Some(0)),
        &adapter,
        &CommandContext::default(),
    )
    .expect("find should succeed");

    let found = names(&response);
    assert_eq!(found.len(), SUBSTRING_MATCHES + 1);
    assert_eq!(found[0], "Save");
    assert_eq!(found[1], "Save As 0");
}

#[test]
fn the_default_limit_keeps_an_exact_name_that_traversal_reaches_after_the_cut() {
    let _guard = HomeGuard::new();
    let adapter = LiveFindAdapter::with_buttons(&longer_names_then_the_exact_one());

    let response = execute(
        find_by_name("Save", None),
        &adapter,
        &CommandContext::default(),
    )
    .expect("find should succeed");

    let found = names(&response);
    assert_eq!(found.len(), DEFAULT_LIMIT);
    assert_eq!(found[0], "Save");
    assert_eq!(found[1], "Save As 0");
    assert_eq!(
        found[DEFAULT_LIMIT - 1],
        format!("Save As {}", DEFAULT_LIMIT - 2)
    );
    assert_eq!(response["total_matches"], SUBSTRING_MATCHES + 1);
    assert_eq!(response["truncated"], true);
}

#[test]
fn without_an_exact_name_the_limit_keeps_the_first_matches_in_traversal_order() {
    let _guard = HomeGuard::new();
    let adapter = LiveFindAdapter::with_buttons(&longer_names_then_the_exact_one());

    let response = execute(
        find_by_name("Save As", None),
        &adapter,
        &CommandContext::default(),
    )
    .expect("find should succeed");

    let expected = (0..DEFAULT_LIMIT)
        .map(|index| format!("Save As {index}"))
        .collect::<Vec<_>>();
    assert_eq!(names(&response), expected);
    assert_eq!(response["total_matches"], SUBSTRING_MATCHES);
    assert_eq!(response["truncated"], true);
}

fn single_by_name(name: &str, pick: fn(&mut FindSelectionArgs)) -> FindArgs {
    let mut args = find_by_name(name, None);
    pick(&mut args.selection);
    args
}

fn single_name(args: FindArgs, adapter: &LiveFindAdapter) -> String {
    let response =
        execute(args, adapter, &CommandContext::default()).expect("find should succeed");
    response["match"]["name"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_default()
}

#[test]
fn first_returns_the_exact_name_over_a_longer_name_found_earlier() {
    let _guard = HomeGuard::new();
    let adapter = LiveFindAdapter::with_buttons(&longer_names_then_the_exact_one());

    let found = single_name(
        single_by_name("Save", |selection| selection.first = true),
        &adapter,
    );

    assert_eq!(found, "Save");
}

#[test]
fn first_without_an_exact_name_returns_the_first_match_in_traversal_order() {
    let _guard = HomeGuard::new();
    let adapter = LiveFindAdapter::with_buttons(&longer_names_then_the_exact_one());

    let found = single_name(
        single_by_name("Save As", |selection| selection.first = true),
        &adapter,
    );

    assert_eq!(found, "Save As 0");
}

#[test]
fn last_and_nth_keep_traversal_order() {
    let _guard = HomeGuard::new();
    let adapter = LiveFindAdapter::with_buttons(&longer_names_then_the_exact_one());

    let nth = single_name(
        single_by_name("Save", |selection| selection.nth = Some(0)),
        &adapter,
    );
    let last = single_name(
        single_by_name("Save As", |selection| selection.last = true),
        &adapter,
    );

    assert_eq!(nth, "Save As 0");
    assert_eq!(last, format!("Save As {}", SUBSTRING_MATCHES - 1));
}
