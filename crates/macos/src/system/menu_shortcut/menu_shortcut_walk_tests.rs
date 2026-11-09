use super::*;

fn pending(depth: u8, menu: Option<&str>) -> Pending {
    Pending {
        element: crate::tree::element_for_pid(i32::MAX),
        depth,
        menu: menu.map(str::to_string),
        parent: None,
        position: 4,
    }
}

fn nested(menu: &str, parent: &str, position: usize) -> Pending {
    Pending {
        parent: Some(parent.into()),
        position,
        ..pending(5, Some(menu))
    }
}

#[test]
fn a_read_is_named_by_its_menu_its_holder_and_its_title_or_position() {
    assert_eq!(read_label(&pending(3, Some("File")), None), "File > item 4");
    assert_eq!(
        read_label(&pending(3, Some("File")), Some("")),
        "File > item 4"
    );
    assert_eq!(
        read_label(&pending(3, Some("Edit")), Some("AutoFill")),
        "Edit > AutoFill"
    );
    assert_eq!(
        read_label(&nested("File", "Open Recent", 2), None),
        "File > Open Recent > item 2"
    );
    assert_eq!(
        read_label(&nested("Edit", "Edit", 6), Some("Speech")),
        "Edit > Speech"
    );
    assert_eq!(read_label(&pending(1, None), None), "menu bar > item 4");
}

#[test]
fn an_item_whose_own_read_failed_is_recorded_by_position() {
    let mut walk = MenuWalk::default();
    step_over(busy(), &nested("Format", "Font", 3), None, &mut walk).unwrap();
    assert_eq!(walk.unread_menus, ["Format > Font > item 3"]);
}

fn busy() -> AdapterError {
    child_error("child_read")
}

#[test]
fn an_unreadable_submenu_is_recorded_and_stepped_over() {
    let mut walk = MenuWalk::default();
    step_over(
        busy(),
        &pending(3, Some("Edit")),
        Some("AutoFill"),
        &mut walk,
    )
    .unwrap();
    step_over(
        busy(),
        &pending(5, Some("Format")),
        Some("Writing Direction"),
        &mut walk,
    )
    .unwrap();
    assert_eq!(
        walk.unread_menus,
        ["Edit > AutoFill", "Format > Writing Direction"]
    );
}

#[test]
fn an_unreadable_menu_bar_or_top_level_menu_ends_the_walk_and_says_where() {
    for depth in [0, 1, 2] {
        let mut walk = MenuWalk {
            visited: 7,
            ..MenuWalk::default()
        };
        let error = step_over(busy(), &pending(depth, Some("File")), None, &mut walk).unwrap_err();
        let details = error.details.unwrap();
        assert_eq!(details["kind"], "child_read", "{depth}");
        assert_eq!(details["depth"], depth);
        assert_eq!(details["menu"], "File");
        assert_eq!(details["visited"], 7);
        assert!(walk.unread_menus.is_empty());
    }
}

#[test]
fn a_permission_failure_inside_a_submenu_is_never_stepped_over() {
    let mut walk = MenuWalk::default();
    let denied = AdapterError::new(ErrorCode::PermDenied, "no accessibility");
    let error = step_over(denied, &pending(4, Some("Edit")), None, &mut walk).unwrap_err();
    assert_eq!(error.code, ErrorCode::PermDenied);
}

#[test]
fn only_menu_roles_are_descended() {
    for role in ["AXMenuBar", "AXMenuBarItem", "AXMenu", "AXMenuItem"] {
        assert!(holds_menu_items(Some(role)), "{role}");
    }
    for role in ["AXTextField", "AXGroup", "AXStaticText"] {
        assert!(!holds_menu_items(Some(role)), "{role}");
    }
    assert!(!holds_menu_items(None));
}

#[test]
fn submenu_reads_get_a_short_slice_and_top_level_reads_the_whole_budget() {
    let lookup = Deadline::from_duration(Duration::from_millis(1_000)).unwrap();
    for depth in [0, 1, 2] {
        assert!(
            read_slice(lookup, depth).remaining() > SUBMENU_READ_SLICE,
            "{depth}"
        );
    }
    for depth in [3, 4, 7] {
        assert!(
            read_slice(lookup, depth).remaining() <= SUBMENU_READ_SLICE,
            "{depth}"
        );
    }
}
