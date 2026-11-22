use agent_desktop_core::{ObservationRequest, SnapshotSurface, TreeOptions};

use super::*;
use crate::tree::Field;
use crate::tree::live::test_support::read_of;
use crate::tree::vocabulary::role_numbers::{FRAME, MENU_ITEM};
use crate::tree::vocabulary::state_bits::VISIBLE;

fn fixture(opener: Field<Vec<u32>>, item: Option<u32>) -> (Walk<ObservedRead>, Vec<u32>) {
    fixture_under(MENU_BAR, opener, item)
}

fn fixture_under(
    parent_role: u32,
    opener: Field<Vec<u32>>,
    item: Option<u32>,
) -> (Walk<ObservedRead>, Vec<u32>) {
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
                    let (role, name, children) = match *key {
                        0 => (FRAME, "Window", vec![1]),
                        1 => (parent_role, "Menu bar", vec![2, 3]),
                        2 => (MENU, "File", vec![4, 5]),
                        3 => (MENU_ITEM, "Edit", vec![6]),
                        4 => (MENU_ITEM, "Open", Vec::new()),
                        5 => (MENU, "Recent", vec![7]),
                        6 => (MENU_ITEM, "Copy", Vec::new()),
                        _ => (MENU_ITEM, "Document", Vec::new()),
                    };
                    let mut read = read_of(role, name, &[]);
                    read.node.traits.states = if *key == 2 {
                        opener.clone()
                    } else {
                        Field::Present(vec![1 << SHOWING, 0])
                    };
                    Expansion::read(read, children)
                })
                .collect())
        },
        |children| {
            children.iter().all(|key| {
                let state = if *key == 4 { item } else { Some(0) };
                is_hidden_state(
                    state
                        .map(|bits| [bits, 0])
                        .as_ref()
                        .map(<[u32; 2]>::as_slice),
                )
            })
        },
    )
    .unwrap();
    (walk, visited)
}

#[test]
fn closed_menu_descendants_are_never_walked() {
    let (walk, visited) = fixture(Field::Present(vec![1 << SHOWING, 0]), Some(1 << VISIBLE));
    assert_eq!(visited, [0, 1, 2, 3]);
    assert!(walk.complete);
    assert!(
        walk.root.children[0]
            .children
            .iter()
            .all(|menu| !menu.truncated)
    );
}

#[test]
fn a_showing_item_keeps_the_whole_open_menu() {
    let (_, visited) = fixture(
        Field::Present(vec![1 << SHOWING, 0]),
        Some((1 << SHOWING) | (1 << VISIBLE)),
    );
    assert_eq!(visited, [0, 1, 2, 3, 4, 5, 7]);
}

#[test]
fn an_expanded_menu_keeps_its_descendants() {
    let (_, visited) = fixture(Field::Present(vec![1 << EXPANDED, 0]), Some(0));
    assert_eq!(visited, [0, 1, 2, 3, 4, 5, 7]);
}

#[test]
fn menubar_openers_stay_in_the_tree_when_closed() {
    let (walk, _) = fixture(Field::Present(vec![1 << SHOWING, 0]), Some(0));
    let bar = &walk.root.children[0];
    let openers: Vec<_> = bar
        .children
        .iter()
        .map(|node| {
            node.payload
                .node
                .properties
                .name
                .present()
                .unwrap()
                .as_str()
        })
        .collect();
    assert_eq!(openers, ["File", "Edit"]);
}

#[test]
fn unreadable_menu_state_keeps_its_descendants() {
    let (_, visited) = fixture(Field::Failed("GetState failed".to_owned()), Some(0));
    assert_eq!(visited, [0, 1, 2, 3, 4, 5, 7]);
}

#[test]
fn empty_menu_state_keeps_its_descendants() {
    let (_, visited) = fixture(Field::Present(Vec::new()), Some(0));
    assert_eq!(visited, [0, 1, 2, 3, 4, 5, 7]);
}

#[test]
fn unreadable_child_state_keeps_the_menu_descendants() {
    let (_, visited) = fixture(Field::Present(vec![1 << SHOWING, 0]), None);
    assert_eq!(visited, [0, 1, 2, 3, 4, 5, 7]);
}

#[test]
fn menus_outside_a_menubar_keep_their_descendants() {
    let (_, visited) = fixture_under(FRAME, Field::Present(vec![1 << SHOWING, 0]), Some(0));
    assert_eq!(visited, [0, 1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn menu_policy_applies_only_to_window_observations() {
    for (surface, expected) in [
        (SnapshotSurface::Window, true),
        (SnapshotSurface::Menu, false),
        (SnapshotSurface::Menubar, false),
    ] {
        let options = TreeOptions {
            surface,
            ..TreeOptions::default()
        };
        let request = ObservationRequest::snapshot(&options, Deadline::after(5_000).unwrap());
        assert_eq!(
            WalkLimits::for_request(&request)
                .children
                .prune_closed_menus,
            expected
        );
    }
}

fn twin_fixture(bounds: Field<crate::tree::Extents>, presented: bool) -> Walk<ObservedRead> {
    walk_menu_levels(
        0,
        &WalkLimits::default(),
        Deadline::standard().unwrap(),
        |level| {
            Ok(level
                .iter()
                .map(|key| {
                    let (role, children) = match *key {
                        0 => (FRAME, if presented { vec![1, 2] } else { vec![1] }),
                        1 => (MENU_BAR, vec![3]),
                        2 => (crate::tree::vocabulary::role_numbers::PANEL, vec![4]),
                        4 => (MENU_BAR, vec![]),
                        _ => (MENU_ITEM, vec![]),
                    };
                    let mut read = read_of(role, "", &[]);
                    read.node.extents = if *key == 1 {
                        bounds.clone()
                    } else {
                        Field::Present(crate::tree::Extents {
                            x: 0,
                            y: 0,
                            width: 100,
                            height: 24,
                        })
                    };
                    Expansion::read(read, children)
                })
                .collect())
        },
        |_| false,
    )
    .unwrap()
}

#[test]
fn only_zero_area_menubar_is_kept() {
    for (width, height) in [(0, 0), (100, 0), (0, 24)] {
        let walk = twin_fixture(
            Field::Present(crate::tree::Extents {
                x: 0,
                y: 0,
                width,
                height,
            }),
            false,
        );
        assert_eq!(walk.root.children.len(), 1);
        assert_eq!(walk.root.children[0].children.len(), 1);
    }
}

#[test]
fn zero_area_twin_is_dropped_when_presented_bar_is_deeper() {
    let walk = twin_fixture(
        Field::Present(crate::tree::Extents {
            x: 0,
            y: 0,
            width: 100,
            height: 0,
        }),
        true,
    );
    assert_eq!(walk.root.children.len(), 1);
    assert_eq!(walk.root.children[0].source_index, 1);
    assert_eq!(walk.root.children[0].children.len(), 1);
}

#[test]
fn unknown_menubar_extents_are_kept_even_with_presented_bar() {
    for bounds in [Field::Absent, Field::Failed("unavailable".to_owned())] {
        let walk = twin_fixture(bounds, true);
        assert_eq!(walk.root.children.len(), 2);
        assert_eq!(walk.root.children[0].children.len(), 1);
    }
}
