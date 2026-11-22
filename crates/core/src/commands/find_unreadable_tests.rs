use super::test_support::LiveFindAdapter;
use super::*;
use crate::{
    AdapterError, AppInfo, Deadline, ObservationRoot, SnapshotSurface, WindowFilter, WindowInfo,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::live_locator::{
        LocatorStats, ObservationRequest, ObservationSource, ObservedSubtree, ObservedTree,
    },
    refs::refs_test_support::HomeGuard,
};

/// A menubar whose Window menu is read and whose Help menu holds a field the
/// app will not read out, then a readable item, with no budget limit or
/// transient cause recorded.
struct UnreadableMenubar(bool);

fn app() -> AppInfo {
    AppInfo {
        name: "MenuApp".into(),
        pid: crate::ProcessId::new(42),
        bundle_id: Some("org.example.menu".into()),
        process_instance: Some("instance-42".into()),
        presentation: None,
    }
}

fn node(role: &str, name: Option<&str>, children: Vec<ObservedSubtree>) -> ObservedSubtree {
    ObservedSubtree::new(LiveFindAdapter::evidence(role, name), children, true, None)
}

impl ObservationOps for UnreadableMenubar {
    fn is_application_surface(&self, surface: SnapshotSurface) -> bool {
        surface == SnapshotSurface::Menubar
    }

    fn focused_app(&self, _: Deadline) -> Result<Option<AppInfo>, AdapterError> {
        Ok(Some(app()))
    }

    fn list_apps(&self, _: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(vec![app()])
    }

    fn list_windows(&self, _: &WindowFilter, _: Deadline) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(Vec::new())
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let mut calculator = node("menuitem", Some("Calculator"), Vec::new());
        if self.0 {
            calculator.evidence.identifiers = calculator
                .evidence
                .identifiers
                .with_retained_object("axtok1:calculator".into());
        }
        let window_menu = node(
            "menuitem",
            Some("Window"),
            vec![node("menu", Some("Window"), vec![calculator])],
        );
        let unread_field = ObservedSubtree::new(
            LiveFindAdapter::evidence("textfield", None),
            Vec::new(),
            false,
            None,
        );
        let help_menu = ObservedSubtree::new(
            LiveFindAdapter::evidence("menuitem", Some("Help")),
            vec![ObservedSubtree::new(
                LiveFindAdapter::evidence("menu", Some("Help")),
                vec![
                    unread_field,
                    node("menuitem", Some("About MenuApp"), Vec::new()),
                ],
                false,
                None,
            )],
            false,
            None,
        );
        let menubar = ObservedSubtree::new(
            LiveFindAdapter::evidence("menu", None),
            vec![window_menu, help_menu],
            false,
            None,
        );
        ObservedTree::from_roots(
            vec![menubar],
            ObservationSource::from_root(&root, request.surface),
            LocatorStats::default(),
            false,
        )
    }
}

impl ActionOps for UnreadableMenubar {}
impl InputOps for UnreadableMenubar {}
impl SystemOps for UnreadableMenubar {
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![SnapshotSurface::Window, SnapshotSurface::Menubar]
    }
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

fn find_args(
    surface: SnapshotSurface,
    role: Option<&str>,
    name: &str,
    selection: FindSelectionArgs,
) -> FindArgs {
    FindArgs {
        scope: FindScopeArgs {
            app: (surface == SnapshotSurface::Menubar).then(|| "MenuApp".into()),
            window_id: (surface == SnapshotSurface::Window).then(|| "w-1".into()),
            root: None,
            surface,
            scroll: None,
        },
        filter: FindFilterArgs {
            name: Some(name.into()),
            role: role.map(str::to_string),
            description: None,
            native_id: None,
            value: None,
            text: None,
            exact: false,
        },
        states: Vec::new(),
        timeout_ms: None,
        selection,
    }
}

fn menubar_find(
    role: Option<&str>,
    name: &str,
    selection: FindSelectionArgs,
) -> Result<Value, AppError> {
    execute(
        find_args(SnapshotSurface::Menubar, role, name, selection),
        &UnreadableMenubar(false),
        &CommandContext::default(),
    )
}

fn unreadable_details(error: AppError) -> serde_json::Value {
    let AppError::Adapter(error) = error else {
        panic!("expected adapter error");
    };
    let suggestion = error.suggestion.clone().unwrap_or_default();
    assert!(!error.permits_retry_by_default());
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
    assert!(
        !suggestion.contains("Raise --timeout-ms"),
        "an unreadable part is not a budget problem: {suggestion}"
    );
    error.details.expect("details")
}

#[test]
fn a_menubar_match_seen_beside_an_unreadable_part_is_listed_and_marked_incomplete() {
    let _guard = HomeGuard::new();
    let response = menubar_find(None, "Calculator", listing())
        .expect("an observed match is an answer, not a timeout");

    assert_eq!(response["total_matches"], 1);
    assert_eq!(response["matches"][0]["name"], "Calculator");
    assert_eq!(response["complete"], false);
    assert_eq!(response["truncated"], true);
    assert!(response.get("hint").is_none());
    assert!(response["matches"][0]["ref_id"].is_null());
    assert!(response.get("snapshot_id").is_none());
}

#[test]
fn a_token_backed_menubar_ref_keeps_its_incomplete_capture_verdict() {
    let _guard = HomeGuard::new();
    let context = CommandContext::default();
    let response = execute(
        find_args(SnapshotSurface::Menubar, None, "Calculator", listing()),
        &UnreadableMenubar(true),
        &context,
    )
    .unwrap();
    let ref_id = response["matches"][0]["ref_id"]
        .as_str()
        .expect("token ref");
    let entry = crate::commands::helpers::load_ref_entry(ref_id, &context).unwrap();
    assert_eq!(response["complete"], false);
    assert_eq!(entry.identity.capture, crate::CaptureVerdict::Incomplete);
    assert_eq!(
        crate::refs::ref_identity::element_token(&entry),
        Some("axtok1:calculator")
    );
}

#[test]
fn no_menubar_match_beside_an_unreadable_part_refuses_without_claiming_absence() {
    let _guard = HomeGuard::new();
    for role in [None, Some("menuitem")] {
        let error = menubar_find(role, "Missing", listing())
            .expect_err("the unread part could hold the match, whatever the role filter");
        assert_eq!(error.code(), "ELEMENT_NOT_FOUND", "role {role:?}");
        let details = unreadable_details(error);
        assert_eq!(details["kind"], "locator_unreadable_part");
        assert_eq!(details["complete"], false);
        assert_eq!(details["observed_matches"], 0);
    }
}

#[test]
fn a_menubar_first_match_read_before_the_unreadable_part_is_answered() {
    let _guard = HomeGuard::new();
    let response = menubar_find(
        None,
        "Calculator",
        FindSelectionArgs {
            first: true,
            ..listing()
        },
    )
    .expect("every node before the match was read, so it is proven first");
    assert_eq!(response["match"]["name"], "Calculator");
    assert_ne!(response["complete"], false);
}

#[test]
fn a_menubar_pick_the_unreadable_part_could_change_stays_refused() {
    let _guard = HomeGuard::new();
    let first = FindSelectionArgs {
        first: true,
        ..listing()
    };
    for (name, selection) in [
        ("About MenuApp", first),
        (
            "Calculator",
            FindSelectionArgs {
                count: true,
                ..listing()
            },
        ),
        (
            "Calculator",
            FindSelectionArgs {
                last: true,
                ..listing()
            },
        ),
        (
            "Calculator",
            FindSelectionArgs {
                nth: Some(0),
                ..listing()
            },
        ),
    ] {
        let error = menubar_find(None, name, selection)
            .expect_err("a match in the unread part would change the answer");
        assert_eq!(error.code(), "ELEMENT_NOT_FOUND", "{name}");
        let AppError::Adapter(ref adapter_error) = error else {
            panic!("adapter error")
        };
        let suggestion = adapter_error.suggestion.as_deref().unwrap();
        assert!(suggestion.contains("--first") && suggestion.contains("--count"));
        let details = unreadable_details(error);
        assert_eq!(details["kind"], "locator_unreadable_part");
        assert_eq!(details["observed_matches"], 1, "{name}");
    }
}

/// The unreadable-part test reads `query_stats` from serialized details, so
/// the key names it reads are pinned against the typed stats here.
#[test]
fn every_budget_limit_and_transient_read_counter_rules_out_an_unreadable_part() {
    let details = |stats: &LocatorStats| {
        AdapterError::timeout("incomplete").with_details(serde_json::json!({
            "kind": "locator_incomplete",
            "retryable": false,
            "query_stats": stats,
        }))
    };
    assert!(super::left_a_part_unreadable(&details(
        &LocatorStats::default()
    )));
    let counters: [fn(&mut LocatorStats); 9] = [
        |stats| stats.traversal.limits.node_hits = 1,
        |stats| stats.traversal.limits.edge_hits = 1,
        |stats| stats.traversal.limits.child_hits = 1,
        |stats| stats.traversal.limits.text_hits = 1,
        |stats| stats.traversal.limits.depth_hits = 1,
        |stats| stats.traversal.limits.child_count_changes = 1,
        |stats| stats.reads.health.cannot_complete = 1,
        |stats| stats.reads.health.deadline_exhausted = 1,
        |stats| stats.reads.health.invalidated_elements = 1,
    ];
    for (index, set) in counters.iter().enumerate() {
        let mut stats = LocatorStats::default();
        set(&mut stats);
        assert!(
            !super::left_a_part_unreadable(&details(&stats)),
            "counter {index} must rule out an unreadable part"
        );
    }
}

#[test]
fn allow_partial_keeps_its_own_contract_on_the_menubar() {
    let _guard = HomeGuard::new();
    let response = menubar_find(
        None,
        "Missing",
        FindSelectionArgs {
            allow_partial: true,
            ..listing()
        },
    )
    .expect("allow-partial answers for the readable part");
    assert_eq!(response["total_matches"], 0);
    assert_eq!(response["complete"], false);
}

#[test]
fn a_window_find_beside_an_unreadable_part_keeps_the_strict_contract() {
    let _guard = HomeGuard::new();
    for name in ["OnlyInWindowOne", "missing"] {
        let error = execute(
            find_args(SnapshotSurface::Window, None, name, listing()),
            &LiveFindAdapter::incomplete(),
            &CommandContext::default(),
        )
        .expect_err("only the menubar answers with observed matches");
        assert_eq!(error.code(), "TIMEOUT", "{name}");
        let AppError::Adapter(error) = error else {
            panic!("expected adapter error");
        };
        assert_eq!(
            error.details.unwrap()["kind"],
            "locator_incomplete",
            "{name}"
        );
    }
}
