use super::*;
use crate::tree::vocabulary::state_bits::SHOWING;

#[test]
fn mixed_child_visibility_reaches_list_evidence_before_output_projection() {
    for role in [31, 55, 65, 66, 98, 121] {
        let limits = WalkLimits::default();
        let edges = HashMap::from([(0, vec![1, 2])]);
        let walked = walk(
            &edges,
            |key| {
                let mut read = read_of(if key == 0 { role } else { 32 }, "", &[]);
                read.node.traits.states =
                    Field::Present(vec![if key == 2 { 0 } else { 1 << SHOWING }, 0]);
                read
            },
            &limits,
        );
        assert_eq!(walked.root.children.len(), 2);
        let tree = tree_of(walked, &limits);
        assert_eq!(tree.presentation.scrolls_vertically, Some(true), "{role}");
    }
}

#[test]
fn only_offscreen_items_not_headers_or_horizontal_bars_indicate_overflow() {
    for (role, expected) in [
        (32, Some(true)),
        (56, Some(true)),
        (90, Some(true)),
        (91, Some(true)),
        (47, None),
        (57, None),
        (58, None),
        (48, None),
        (29, None),
    ] {
        let limits = WalkLimits::default();
        let walked = walk(
            &HashMap::from([(0, vec![1, 2])]),
            |key| {
                let mut read = read_of(
                    match key {
                        0 => 31,
                        1 => 32,
                        _ => role,
                    },
                    "",
                    &[],
                );
                read.node.traits.states =
                    Field::Present(vec![if key == 2 { 0 } else { 1 << SHOWING }, 0]);
                read
            },
            &limits,
        );
        assert_eq!(
            tree_of(walked, &limits).presentation.scrolls_vertically,
            expected,
            "role={role}"
        );
    }
}

#[test]
fn row_visibility_belongs_to_the_lists_own_viewport_not_an_outer_page() {
    let viewport = Extents {
        x: 0,
        y: 0,
        width: 400,
        height: 300,
    };
    for (bounds, expected) in [
        (Some(viewport), Some(true)),
        (
            Some(Extents {
                y: 200,
                height: 200,
                ..viewport
            }),
            None,
        ),
        (None, None),
        (
            Some(Extents {
                width: 0,
                height: 0,
                ..viewport
            }),
            None,
        ),
    ] {
        let limits = WalkLimits::default();
        let walked = walk(
            &HashMap::from([(0, vec![1]), (1, vec![2]), (2, vec![3, 4])]),
            |key| {
                let mut read = read_of(
                    match key {
                        0 => 49,
                        1 => 39,
                        2 => 31,
                        _ => 32,
                    },
                    "",
                    &[],
                );
                read.node.traits.states =
                    Field::Present(vec![if key == 4 { 0 } else { 1 << SHOWING }, 0]);
                read.node.extents = match key {
                    0 => Field::Present(viewport),
                    2 => bounds.map_or(Field::Absent, Field::Present),
                    _ => Field::Absent,
                };
                read
            },
            &limits,
        );
        let tree = tree_of(walked, &limits);
        assert_eq!(
            tree.children[0].children[0].presentation.scrolls_vertically, expected,
            "bounds={bounds:?}"
        );
    }
}
