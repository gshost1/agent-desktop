use super::*;
use agent_desktop_core::{Action, ActionStep, StepMechanism};

struct Node {
    role: &'static str,
    subrole: Option<&'static str>,
    parent: Option<usize>,
}

fn walk(nodes: &[Node], start: usize) -> Option<(usize, &'static str)> {
    enclosing_surface(
        &start,
        |index| nodes.get(*index).map(|node| node.role.to_string()),
        |index| nodes.get(*index)?.subrole.map(str::to_string),
        |index| nodes.get(*index)?.parent,
    )
}

fn node(role: &'static str, subrole: Option<&'static str>, parent: Option<usize>) -> Node {
    Node {
        role,
        subrole,
        parent,
    }
}

fn button_under(container: Node) -> Vec<Node> {
    vec![
        node("AXButton", None, Some(1)),
        node("AXGroup", Some("AXApplicationAlertDialog"), Some(2)),
        container,
        node("AXWindow", Some("AXStandardWindow"), Some(4)),
        node("AXApplication", None, None),
    ]
}

#[test]
fn a_button_inside_a_sheet_watches_that_sheet() {
    let nodes = button_under(node("AXSheet", None, Some(3)));
    assert_eq!(walk(&nodes, 0), Some((2, SHEET)));
}

#[test]
fn a_window_that_is_a_sheet_or_dialog_is_the_surface() {
    for (subrole, kind) in [
        ("AXSheet", SHEET),
        ("AXDialog", ALERT),
        ("AXSystemDialog", ALERT),
        ("AXAlert", ALERT),
    ] {
        let nodes = vec![
            node("AXButton", None, Some(1)),
            node("AXWindow", Some(subrole), Some(2)),
            node("AXApplication", None, None),
        ];
        assert_eq!(walk(&nodes, 0), Some((1, kind)), "{subrole}");
    }
}

#[test]
fn a_press_in_a_standard_window_has_no_surface_to_watch() {
    let nodes = button_under(node("AXGroup", None, Some(3)));
    assert_eq!(walk(&nodes, 0), None);
    let floating = vec![
        node("AXButton", None, Some(1)),
        node("AXWindow", Some("AXFloatingWindow"), None),
    ];
    assert_eq!(walk(&floating, 0), None);
}

#[test]
fn a_popover_or_menu_inside_a_sheet_ends_the_walk() {
    for stop in ["AXPopover", "AXMenu", "AXMenuItem"] {
        let nodes = vec![
            node("AXButton", None, Some(1)),
            node(stop, None, Some(2)),
            node("AXSheet", None, Some(3)),
            node("AXWindow", Some("AXStandardWindow"), None),
        ];
        assert_eq!(walk(&nodes, 0), None, "{stop}");
    }
}

#[test]
fn an_unreadable_ancestry_or_a_deep_tree_is_no_surface() {
    let broken = vec![node("AXButton", None, Some(9))];
    assert_eq!(walk(&broken, 0), None);
    let mut deep: Vec<Node> = (0..SURFACE_ANCESTOR_WALK)
        .map(|index| node("AXGroup", None, Some(index + 1)))
        .collect();
    deep.push(node("AXSheet", None, None));
    assert_eq!(walk(&deep, 0), None);
}

fn pressed() -> ActionResult {
    ActionResult::from_execution(
        &Action::Click,
        vec![
            ActionStep::succeeded("AXPress")
                .with_mechanism(StepMechanism::SemanticApi)
                .with_verified(false),
        ],
    )
}

fn surface(window: Option<i64>) -> PressedSurface {
    PressedSurface {
        kind: SHEET,
        window,
    }
}

#[test]
fn a_closed_surface_verifies_the_press_and_names_the_signal() {
    let result = surface(Some(7)).report(
        pressed(),
        Effect::Closed(window_surface_watch::SURFACE_WINDOW_CLOSED),
    );
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["disposition"]["delivery"], "delivered_verified");
    assert_eq!(value["steps"][0]["verified"], true);
    assert_eq!(value["details"][DETAIL]["surface"], "sheet");
    assert_eq!(value["details"][DETAIL]["effect"], "closed");
    assert_eq!(value["details"][DETAIL]["signal"], "surface_window_closed");
}

#[test]
fn a_surface_still_showing_leaves_the_press_unverified_and_says_so() {
    let result = surface(Some(7)).report(pressed(), Effect::StillShowing);
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
    assert_eq!(value["steps"][0]["verified"], false);
    assert_eq!(value["details"][DETAIL]["effect"], "still_showing");
    assert!(value["details"][DETAIL].get("signal").is_none());
}

#[test]
fn a_surface_without_its_own_window_is_reported_unobservable_without_waiting() {
    let started = std::time::Instant::now();
    let result = surface(None).judge(pressed(), Deadline::after(30_000).unwrap());
    assert!(started.elapsed() < Duration::from_millis(50));
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["disposition"]["delivery"], "delivered_unverified");
    assert_eq!(value["details"][DETAIL]["effect"], "unobservable");
}

#[test]
fn a_verified_or_undelivered_press_is_never_judged() {
    let verified = pressed().with_verified_delivery();
    let judged = surface(Some(7)).judge(verified, Deadline::after(30_000).unwrap());
    assert!(judged.details.is_none());
    let skipped = ActionResult::satisfied_without_delivery("click");
    let judged = surface(Some(7)).judge(skipped, Deadline::after(30_000).unwrap());
    assert!(judged.details.is_none());
}

#[test]
fn the_report_keeps_details_the_press_already_carried() {
    let result = pressed().with_details(serde_json::json!({ "kept": 1 }));
    let value =
        serde_json::to_value(surface(Some(7)).report(result, Effect::StillShowing)).unwrap();
    assert_eq!(value["details"]["kept"], 1);
    assert_eq!(value["details"][DETAIL]["effect"], "still_showing");
}
