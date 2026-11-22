use super::*;
use crate::tree::live::test_support::read_of;
use crate::tree::vocabulary::role_numbers::{FRAME, PANEL};
use crate::tree::vocabulary::state_bits::VISIBLE;
use crate::tree::{Extents, Field};

fn extents(width: i32, height: i32) -> Field<Extents> {
    Field::Present(Extents {
        x: 0,
        y: 17,
        width,
        height,
    })
}

#[test]
fn zero_area_menubar_excludes_the_whole_branch_even_when_showing() {
    for excluded_bounds in [extents(1280, 0), extents(0, 24), extents(0, 0)] {
        let mut visited = Vec::new();
        let walk = walk_menu_levels(
            0,
            &WalkLimits::default(),
            Deadline::after(5_000).unwrap(),
            |level| {
                visited.extend_from_slice(level);
                Ok(level
                    .iter()
                    .map(|key| {
                        let (role, name, bounds, children) = match *key {
                            0 => (FRAME, "Window", extents(1280, 800), vec![1, 2]),
                            1 => (MENU_BAR, "", excluded_bounds.clone(), vec![3]),
                            2 => (MENU_BAR, "", extents(1280, 24), vec![4]),
                            _ => (MENU, "Format", extents(60, 24), vec![]),
                        };
                        let mut read = read_of(role, name, &[]);
                        read.node.extents = bounds;
                        read.node.traits.states =
                            Field::Present(vec![(1 << SHOWING) | (1 << VISIBLE), 0]);
                        Expansion::read(read, children)
                    })
                    .collect())
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(visited, [0, 1, 2, 4]);
        assert!(walk.complete);
        let [bar] = walk.root.children.as_slice() else {
            panic!("only the presented bar should be observed");
        };
        assert_eq!(bar.source_index, 1);
        assert_eq!(
            bar.children[0]
                .payload
                .node
                .properties
                .name
                .present()
                .map(String::as_str),
            Some("Format")
        );
    }
}

#[test]
fn unknown_extents_or_non_menubar_role_are_not_evidence_of_suppression() {
    for (role, bounds) in [
        (MENU_BAR, Field::Absent),
        (MENU_BAR, Field::Failed("unavailable".to_owned())),
        (MENU_BAR, extents(-1, 0)),
        (MENU_BAR, extents(1280, 24)),
        (PANEL, extents(1280, 0)),
        (MENU, extents(1280, 0)),
    ] {
        let mut read = read_of(role, "", &[]);
        read.node.extents = bounds;
        assert!(
            !is_unpresented_menubar(&read.node),
            "role {role}: {:?}",
            read.node.extents
        );
    }
}
