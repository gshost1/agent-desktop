use super::*;

fn read(number: Option<i64>, kind: &'static str) -> SurfaceRead {
    SurfaceRead {
        number,
        kind,
        title: Some(format!("{kind} title")),
    }
}

fn window(
    number: Option<i64>,
    own: Option<SurfaceRead>,
    children: Vec<SurfaceRead>,
) -> WindowSurfaces {
    WindowSurfaces {
        number,
        own,
        children,
    }
}

fn ids(surfaces: &[SurfaceInfo]) -> Vec<(&str, &str)> {
    surfaces
        .iter()
        .map(|surface| (surface.id.as_str(), surface.kind.as_str()))
        .collect()
}

#[test]
fn a_surface_is_named_by_its_own_window_number_whichever_window_has_focus() {
    let sheet_window = || window(Some(31), Some(read(Some(31), "sheet")), Vec::new());
    let parent = || window(Some(30), None, vec![read(Some(31), "sheet")]);
    let other = || window(Some(40), None, Vec::new());

    let parent_focused = surfaces_from_windows(vec![parent(), other(), sheet_window()]);
    let other_focused = surfaces_from_windows(vec![parent(), other(), other()]);

    assert_eq!(ids(&parent_focused), [("w-31", "sheet")]);
    assert_eq!(ids(&other_focused), [("w-31", "sheet")]);
}

#[test]
fn a_child_without_its_own_window_counts_only_surfaces_of_its_kind() {
    let before = surfaces_from_windows(vec![window(
        Some(30),
        None,
        vec![read(None, "popover"), read(None, "alert")],
    )]);
    let after_sibling_closed =
        surfaces_from_windows(vec![window(Some(30), None, vec![read(None, "popover")])]);

    assert_eq!(
        ids(&before),
        [("w-30/popover/0", "popover"), ("w-30/alert/0", "alert")]
    );
    assert_eq!(ids(&after_sibling_closed), [("w-30/popover/0", "popover")]);
}

#[test]
fn a_child_reporting_its_parents_window_number_is_named_under_the_parent() {
    let surfaces = surfaces_from_windows(vec![window(
        Some(30),
        None,
        vec![read(Some(30), "popover"), read(Some(30), "popover")],
    )]);

    assert_eq!(
        ids(&surfaces),
        [("w-30/popover/0", "popover"), ("w-30/popover/1", "popover")]
    );
}

#[test]
fn a_window_without_a_number_marks_its_surfaces_unread_instead_of_naming_them() {
    let surfaces = surfaces_from_windows(vec![
        window(None, Some(read(None, "sheet")), Vec::new()),
        window(Some(30), None, vec![read(Some(31), "sheet")]),
    ]);

    assert_eq!(
        ids(&surfaces),
        [("w-31", "sheet"), (UNREAD_WINDOW_OWNER_ID, "window")]
    );
    assert_eq!(surfaces[1].unclassified, ["sheet", "popover", "alert"]);
}

#[test]
fn a_window_without_a_number_or_surfaces_leaves_the_inventory_complete() {
    let surfaces = surfaces_from_windows(vec![window(None, None, Vec::new())]);

    assert!(surfaces.is_empty());
}

fn titled(number: Option<i64>, kind: &'static str, title: &str) -> SurfaceRead {
    SurfaceRead {
        number,
        kind,
        title: Some(title.into()),
    }
}

#[test]
fn identically_titled_sheets_on_their_own_windows_each_get_an_entry() {
    let prompt = "Allow remote debugging?";
    let surfaces = surfaces_from_windows(vec![
        window(
            Some(30),
            None,
            vec![
                titled(Some(31), "sheet", prompt),
                titled(Some(32), "sheet", prompt),
                titled(Some(33), "sheet", prompt),
            ],
        ),
        window(
            Some(33),
            Some(titled(Some(33), "sheet", prompt)),
            Vec::new(),
        ),
    ]);

    assert_eq!(
        ids(&surfaces),
        [("w-31", "sheet"), ("w-32", "sheet"), ("w-33", "sheet")]
    );
    assert!(
        surfaces
            .iter()
            .all(|surface| surface.title.as_deref() == Some(prompt))
    );
}

#[test]
fn identically_titled_sheets_without_their_own_windows_each_get_an_ordinal() {
    let prompt = "Allow remote debugging?";
    let surfaces = surfaces_from_windows(vec![window(
        Some(30),
        None,
        vec![
            titled(None, "sheet", prompt),
            titled(Some(30), "sheet", prompt),
        ],
    )]);

    assert_eq!(
        ids(&surfaces),
        [("w-30/sheet/0", "sheet"), ("w-30/sheet/1", "sheet")]
    );
}
