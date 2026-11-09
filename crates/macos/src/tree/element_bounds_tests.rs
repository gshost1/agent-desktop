use super::*;

#[cfg(target_os = "macos")]
#[test]
fn native_bounds_decoder_rejects_invalid_geometry_before_live_or_snapshot_use() {
    use core_graphics::geometry::{CGPoint, CGSize};
    for (point, size) in [
        (CGPoint::new(0.0, 33.0), CGSize::new(-1.0, 0.0)),
        (CGPoint::new(0.0, 0.0), CGSize::new(10.0, -0.5)),
        (CGPoint::new(f64::NAN, 0.0), CGSize::new(10.0, 10.0)),
        (CGPoint::new(0.0, 0.0), CGSize::new(f64::INFINITY, 10.0)),
        (CGPoint::new(0.0, 0.0), CGSize::new(f64::MAX, 10.0)),
    ] {
        assert_eq!(rect_from_parts(point, size), None);
    }
    for size in [CGSize::new(0.0, 0.0), CGSize::new(20.0, 10.0)] {
        let bounds = rect_from_parts(CGPoint::new(-20.0, 33.0), size).unwrap();
        assert_eq!(bounds.x, -20.0);
        assert_eq!(bounds.width, size.width);
        assert_eq!(bounds.height, size.height);
    }
}

fn rect(x: f64, width: f64) -> Rect {
    Rect {
        x,
        y: 0.0,
        width,
        height: 100.0,
    }
}

#[test]
fn only_clipping_containers_narrow_the_viewport() {
    let window = Some(rect(0.0, 500.0));
    assert_eq!(
        clipped_viewport(window, "AXGroup", Some(rect(20.0, 10.0))),
        window
    );
    assert_eq!(
        clipped_viewport(window, "AXScrollArea", Some(rect(20.0, 100.0))),
        Some(rect(20.0, 100.0))
    );
}

#[test]
fn disjoint_viewports_remain_known_empty_through_descendants() {
    let empty = clipped_viewport(
        Some(rect(0.0, 100.0)),
        "AXScrollArea",
        Some(rect(200.0, 100.0)),
    );
    assert_eq!(
        crate::tree::state_reader::offscreen(Some(rect(-50.0, 500.0)), empty),
        Some(true)
    );
    let nested = clipped_viewport(empty, "AXScrollArea", Some(rect(-50.0, 500.0)));
    assert_eq!(
        crate::tree::state_reader::offscreen(Some(rect(-50.0, 500.0)), nested),
        Some(true)
    );
}

#[test]
fn missing_clipping_geometry_is_unknown_not_empty() {
    assert_eq!(
        clipped_viewport(Some(rect(0.0, 100.0)), "AXScrollArea", None),
        None
    );
    assert_eq!(
        clipped_viewport(None, "AXScrollArea", Some(rect(0.0, 100.0))),
        None
    );
}
/// A menu item's `AXTopLevelUIElement` is the menu bar, whose 29-point height
/// clips every open menu item, so the owning container is read from
/// `AXWindow` alone and never from a weaker attribute.
#[test]
fn the_owning_container_is_read_from_the_window_attribute_alone() {
    let mut attributes = Vec::new();
    let container = first_owning_container(|attribute| {
        attributes.push(attribute);
        Ok(None)
    })
    .unwrap();

    assert!(container.is_none());
    assert_eq!(attributes, ["AXWindow"]);
}

#[test]
fn incomplete_window_read_never_falls_through_to_a_weaker_container() {
    for error in [
        accessibility_sys::kAXErrorCannotComplete,
        accessibility_sys::kAXErrorInvalidUIElement,
    ] {
        let calls = std::cell::Cell::new(0);
        let result = first_owning_container(|_| {
            calls.set(calls.get() + 1);
            Err(error)
        });

        let failure = match result {
            Err(failure) => failure,
            Ok(_) => panic!("incomplete AXWindow read must fail"),
        };
        assert_eq!(failure, ("AXWindow", error));
        assert_eq!(calls.get(), 1);
    }
}

fn ancestry(roles: &[&'static str]) -> impl Fn(&usize) -> Result<Option<usize>, AdapterError> {
    let len = roles.len();
    move |index| Ok((index + 1 < len).then_some(index + 1))
}

#[test]
fn a_window_ancestor_stands_in_for_a_missing_window_attribute() {
    let roles = [
        "AXPopUpButton",
        "AXGroup",
        "AXScrollArea",
        "AXSplitGroup",
        "AXWindow",
        "AXApplication",
    ];

    let window = first_window_ancestor(&0, ancestry(&roles), |index| {
        Ok(Some(roles[*index].to_owned()))
    })
    .unwrap();

    assert_eq!(window, Some(4));
}

#[test]
fn a_chain_that_reaches_the_application_has_no_window_ancestor() {
    let roles = [
        "AXMenuItem",
        "AXMenu",
        "AXMenuBarItem",
        "AXMenuBar",
        "AXApplication",
    ];

    let window = first_window_ancestor(&0, ancestry(&roles), |index| {
        Ok(Some(roles[*index].to_owned()))
    })
    .unwrap();

    assert_eq!(window, None);
}

#[test]
fn a_popup_menu_item_inside_a_window_has_no_window_ancestor() {
    let roles = [
        "AXMenuItem",
        "AXMenu",
        "AXPopUpButton",
        "AXGroup",
        "AXWindow",
    ];

    let window = first_window_ancestor(&0, ancestry(&roles), |index| {
        Ok(Some(roles[*index].to_owned()))
    })
    .unwrap();

    assert_eq!(window, None);
}

#[test]
fn an_ancestor_without_a_role_ends_the_search_unknown() {
    let roles = ["AXPopUpButton", "AXGroup", "AXWindow"];

    let window = first_window_ancestor(&0, ancestry(&roles), |index| {
        Ok((*index != 1).then(|| roles[*index].to_owned()))
    })
    .unwrap();

    assert_eq!(window, None);
}

#[test]
fn an_unbounded_ancestry_fails_instead_of_looping() {
    let error = first_window_ancestor(
        &0_usize,
        |index| Ok(Some(index + 1)),
        |_| Ok(Some("AXGroup".to_owned())),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}

#[test]
fn an_own_surface_element_has_no_window_ancestor_even_as_the_start() {
    for start in ["AXMenu", "AXPopover", "AXMenuItem"] {
        let roles = [start, "AXPopUpButton", "AXGroup", "AXWindow"];

        let window = first_window_ancestor(&0, ancestry(&roles), |index| {
            Ok(Some(roles[*index].to_owned()))
        })
        .unwrap();

        assert_eq!(window, None, "{start}");
    }
}

#[test]
fn a_control_inside_a_popover_has_no_window_ancestor() {
    let roles = ["AXButton", "AXGroup", "AXPopover", "AXGroup", "AXWindow"];

    let window = first_window_ancestor(&0, ancestry(&roles), |index| {
        Ok(Some(roles[*index].to_owned()))
    })
    .unwrap();

    assert_eq!(window, None);
}

fn clip(role: &str, bounds: Option<Rect>) -> (String, Option<Rect>) {
    (role.to_owned(), bounds)
}

#[test]
fn a_sheet_wider_than_its_window_keeps_its_far_controls_visible() {
    let window = rect(0.0, 586.0);
    let sheet = rect(-147.0, 880.0);
    let save_button = rect(650.0, 70.0);

    let viewport = viewport_from_clips(window, &[clip("AXSheet", Some(sheet))]);

    assert_eq!(viewport, Some(sheet));
    assert_eq!(
        crate::tree::state_reader::offscreen(Some(save_button), viewport),
        Some(false)
    );
    assert_eq!(
        crate::tree::state_reader::offscreen(Some(rect(740.0, 20.0)), viewport),
        Some(true)
    );
}

#[test]
fn clips_inside_a_sheet_still_narrow_its_viewport() {
    let viewport = viewport_from_clips(
        rect(0.0, 586.0),
        &[
            clip("AXScrollArea", Some(rect(600.0, 100.0))),
            clip("AXSheet", Some(rect(-147.0, 880.0))),
        ],
    );

    assert_eq!(viewport, Some(rect(600.0, 100.0)));
}

#[test]
fn a_sheet_with_unknown_geometry_leaves_the_viewport_unknown() {
    assert_eq!(
        viewport_from_clips(rect(0.0, 586.0), &[clip("AXSheet", None)]),
        None
    );
}

#[test]
fn window_clips_still_intersect_without_a_sheet() {
    let viewport = viewport_from_clips(
        rect(0.0, 500.0),
        &[
            clip("AXScrollArea", Some(rect(400.0, 300.0))),
            clip("AXWindow", Some(rect(0.0, 500.0))),
        ],
    );

    assert_eq!(viewport, Some(rect(400.0, 100.0)));
}

#[test]
fn snapshot_traversal_entering_a_sheet_takes_the_sheet_frame() {
    let sheet = rect(-147.0, 880.0);
    assert_eq!(
        clipped_viewport(Some(rect(0.0, 586.0)), "AXSheet", Some(sheet)),
        Some(sheet)
    );
    assert_eq!(
        clipped_viewport(Some(rect(0.0, 586.0)), "AXPopover", Some(sheet)),
        Some(sheet)
    );
}
