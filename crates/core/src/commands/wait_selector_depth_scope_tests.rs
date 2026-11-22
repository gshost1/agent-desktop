use super::*;

const BURIED_DEPTH: u8 = 22;

/// `leaf` 22 levels below its window inside nested groups, cut at
/// `max_depth` as a platform walk cuts the tree it reads.
pub(super) fn buried(leaf: AccessibilityNode, max_depth: u8) -> AccessibilityNode {
    fn level(depth: u8, leaf: AccessibilityNode, max_depth: u8) -> AccessibilityNode {
        if depth == BURIED_DEPTH {
            return leaf;
        }
        let mut group = node("group", &format!("level {depth}"));
        if depth < max_depth {
            group.children = vec![level(depth + 1, leaf, max_depth)];
        }
        group
    }
    level(1, leaf, max_depth)
}

fn deep(id: &str, field: Field) -> Pane {
    Pane {
        deep: true,
        ..pane(id, OURS, false, field)
    }
}

fn holds_width(node: &Value) -> bool {
    node["name"] == "Width:"
        || node["children"]
            .as_array()
            .is_some_and(|children| children.iter().any(holds_width))
}

/// A window the click opened holds the selector 22 levels down, below the
/// snapshot's default depth of 10.
#[test]
fn a_deep_match_in_a_window_the_action_opened_is_returned() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let entry = window_entry("w-1");
    let before = TwoWindowApp::new(vec![pane("w-1", OURS, true, Field::Nowhere)]);
    let baseline = baseline(&before, &entry);
    assert!(baseline.is_some());
    let app = TwoWindowApp::new(vec![
        pane("w-1", OURS, true, Field::Nowhere),
        deep("w-2", Field::InWindow(Duration::ZERO)),
    ]);

    let result = click_and_wait(&app, &entry, baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "w-2");
    assert!(holds_width(&result["tree"]), "{result:#}");
}

/// The ref's own window closed, so the wait reads every window of its
/// process and finds the selector 22 levels down in one of them.
#[test]
fn a_deep_match_in_another_window_after_the_ref_window_closed_is_returned() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![deep("w-2", Field::InWindow(Duration::ZERO))]);

    let result = click_and_wait(&app, &window_entry("w-1"), None).unwrap();

    assert_eq!(result["window"]["id"], "w-2");
    assert!(holds_width(&result["tree"]), "{result:#}");
}

/// A sheet open on a window of the ref's process holds the selector 22
/// levels down.
#[test]
fn a_deep_match_in_a_sheet_is_returned() {
    let _home = HomeGuard::new();
    let _clock = VirtualClock::freeze();
    let app = TwoWindowApp::new(vec![
        pane("w-1", OURS, true, Field::Nowhere),
        deep("w-2", Field::InSheet(Duration::ZERO)),
    ]);
    let entry = window_entry("w-1");
    let baseline = baseline(&app, &entry);

    let result = click_and_wait(&app, &entry, baseline.as_ref()).unwrap();

    assert_eq!(result["window"]["id"], "w-2");
    assert!(holds_width(&result["tree"]), "{result:#}");
}
