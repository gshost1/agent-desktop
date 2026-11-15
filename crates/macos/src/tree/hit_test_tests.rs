use super::imp::{
    HitClassification, ax_point, classify_relation, decide_hit, ends_clipping_walk, intersect_rects,
    needs_application_retry, remember_ancestor,
};
use crate::tree::AXElement;
use accessibility_sys::AXUIElementCreateSystemWide;
use agent_desktop_core::{Point, Rect};

#[test]
fn self_hit_reaches_target() {
    assert_eq!(
        classify_relation(true, false),
        HitClassification::ReachesTarget
    );
}

#[test]
fn reaches_target_takes_priority_over_ancestor() {
    assert_eq!(
        classify_relation(true, true),
        HitClassification::ReachesTarget
    );
}

#[test]
fn ancestor_hit_is_reported_separately_from_unrelated() {
    assert_eq!(
        classify_relation(false, true),
        HitClassification::AncestorOfTarget
    );
}

#[test]
fn unrelated_hit_is_neither_target_nor_ancestor() {
    assert_eq!(
        classify_relation(false, false),
        HitClassification::Unrelated
    );
}

#[test]
fn target_ancestor_hit_retries_in_application_scope() {
    assert!(needs_application_retry(HitClassification::AncestorOfTarget));
    assert!(!needs_application_retry(HitClassification::ReachesTarget));
    assert!(!needs_application_retry(HitClassification::Unrelated));
}

#[test]
fn external_display_point_stays_in_global_top_left_coordinates() {
    assert_eq!(
        ax_point(&Point {
            x: 2065.0,
            y: 636.0,
        }),
        (2065.0, 636.0)
    );
}

#[test]
fn application_root_completes_the_clipping_walk_without_a_parent() {
    assert!(ends_clipping_walk(Some("AXApplication")));
    assert!(!ends_clipping_walk(Some("AXGroup")));
    assert!(!ends_clipping_walk(None));
}

#[test]
fn clipped_bounds_keep_only_the_visible_part_of_a_partially_shown_target() {
    let target = Rect {
        x: 100.0,
        y: 900.0,
        width: 220.0,
        height: 160.0,
    };
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 950.0,
    };

    assert_eq!(
        intersect_rects(target, viewport),
        Some(Rect {
            x: 100.0,
            y: 900.0,
            width: 220.0,
            height: 50.0,
        })
    );
}

#[test]
fn ancestor_walk_retains_handles_and_detects_the_same_element() {
    let system = AXElement(unsafe { AXUIElementCreateSystemWide() });
    let equivalent = AXElement(unsafe { AXUIElementCreateSystemWide() });
    let mut visited = Vec::new();

    assert!(remember_ancestor(&mut visited, &system));
    assert!(!remember_ancestor(&mut visited, &equivalent));
    assert_eq!(visited.len(), 1);
}

#[test]
fn a_sheet_or_popover_frame_is_the_last_clip_before_its_parent_window() {
    use crate::tree::element_bounds::draws_own_surface;
    assert!(draws_own_surface("AXSheet"));
    assert!(draws_own_surface("AXPopover"));
    assert!(!draws_own_surface("AXWindow"));
    assert!(!draws_own_surface("AXScrollArea"));
}

fn covering_window() -> agent_desktop_core::adapter::hit_test::HitTestResult {
    agent_desktop_core::adapter::hit_test::HitTestResult::InterceptedBy {
        role: Some("window".into()),
        name: None,
        bounds: Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 300.0,
        }),
    }
}

#[test]
fn an_undecided_hit_over_another_apps_window_is_intercepted_by_that_window() {
    use agent_desktop_core::adapter::hit_test::HitTestResult;

    let mut asked_pid = None;
    let result = decide_hit(
        || Ok(HitTestResult::Unknown),
        || Some(4242),
        |pid| {
            asked_pid = Some(pid);
            Some(covering_window())
        },
    )
    .unwrap();

    assert_eq!(result, covering_window());
    assert_ne!(result, HitTestResult::ReachesTarget);
    assert_eq!(asked_pid, Some(4242));
}

#[test]
fn a_decided_accessibility_hit_never_reads_the_window_stack() {
    use agent_desktop_core::adapter::hit_test::HitTestResult;

    let result = decide_hit(
        || Ok(HitTestResult::ReachesTarget),
        || panic!("a decided hit needs no pid"),
        |_| panic!("a decided hit needs no window stack"),
    )
    .unwrap();

    assert_eq!(result, HitTestResult::ReachesTarget);
}

#[test]
fn an_undecided_hit_with_no_covering_window_stays_unknown() {
    use agent_desktop_core::adapter::hit_test::HitTestResult;

    assert_eq!(
        decide_hit(|| Ok(HitTestResult::Unknown), || Some(4242), |_| None).unwrap(),
        HitTestResult::Unknown
    );
    assert_eq!(
        decide_hit(
            || Ok(HitTestResult::Unknown),
            || None,
            |_| panic!("no pid, no window stack read")
        )
        .unwrap(),
        HitTestResult::Unknown
    );
}
