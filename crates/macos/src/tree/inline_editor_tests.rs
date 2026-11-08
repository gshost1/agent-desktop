use super::{EditorFacts, edits_in_place, is_windowless_editor};
use agent_desktop_core::Rect;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Option<Rect> {
    Some(Rect {
        x,
        y,
        width,
        height,
    })
}

fn row_field() -> EditorFacts {
    EditorFacts {
        pid: 42,
        role: Some("AXTextField".into()),
        bounds: rect(100.0, 200.0, 180.0, 18.0),
        has_window: true,
    }
}

fn floating_editor() -> EditorFacts {
    EditorFacts {
        pid: 42,
        role: Some("AXTextField".into()),
        bounds: rect(98.0, 199.0, 120.0, 20.0),
        has_window: false,
    }
}

#[test]
fn a_windowless_focused_editor_over_the_field_edits_it_in_place() {
    assert!(edits_in_place(&row_field(), &floating_editor()));
}

#[test]
fn an_editor_wider_than_its_field_still_covers_it() {
    let editor = EditorFacts {
        bounds: rect(96.0, 198.0, 260.0, 22.0),
        ..floating_editor()
    };
    assert!(edits_in_place(&row_field(), &editor));
}

#[test]
fn each_missing_or_contrary_fact_withholds_the_proof() {
    let field = row_field();
    for (case, editor) in [
        (
            "other process",
            EditorFacts {
                pid: 7,
                ..floating_editor()
            },
        ),
        (
            "editor has a window",
            EditorFacts {
                has_window: true,
                ..floating_editor()
            },
        ),
        (
            "editor is not text",
            EditorFacts {
                role: Some("AXOutline".into()),
                ..floating_editor()
            },
        ),
        (
            "editor role unknown",
            EditorFacts {
                role: None,
                ..floating_editor()
            },
        ),
        (
            "editor bounds unknown",
            EditorFacts {
                bounds: None,
                ..floating_editor()
            },
        ),
        (
            "editor on another row",
            EditorFacts {
                bounds: rect(98.0, 240.0, 120.0, 20.0),
                ..floating_editor()
            },
        ),
        (
            "editor grazes the field",
            EditorFacts {
                bounds: rect(98.0, 214.0, 120.0, 20.0),
                ..floating_editor()
            },
        ),
    ] {
        assert!(!edits_in_place(&field, &editor), "{case}");
    }
}

#[test]
fn only_text_fields_are_edited_in_place() {
    for role in [None, Some("AXButton"), Some("AXRow"), Some("AXStaticText")] {
        let field = EditorFacts {
            role: role.map(str::to_string),
            ..row_field()
        };
        assert!(!edits_in_place(&field, &floating_editor()), "{role:?}");
    }
    let field = EditorFacts {
        bounds: None,
        ..row_field()
    };
    assert!(!edits_in_place(&field, &floating_editor()));
}

#[test]
fn a_focused_editor_inside_a_window_is_an_ordinary_field() {
    assert!(is_windowless_editor(&floating_editor()));
    assert!(!is_windowless_editor(&row_field()));
    assert!(!is_windowless_editor(&EditorFacts {
        role: Some("AXGroup".into()),
        ..floating_editor()
    }));
}

#[test]
fn a_write_aimed_at_the_focused_windowless_editor_is_an_in_place_edit() {
    use super::{InPlaceEdit, classify_edit};

    assert_eq!(
        classify_edit(&floating_editor(), None, true),
        InPlaceEdit::TargetIsEditor
    );
    assert_eq!(
        classify_edit(&row_field(), None, true),
        InPlaceEdit::None,
        "a focused field inside a window is an ordinary field"
    );
    assert_eq!(
        classify_edit(&row_field(), Some(&floating_editor()), false),
        InPlaceEdit::TargetIsShadowed
    );
    assert_eq!(classify_edit(&row_field(), None, false), InPlaceEdit::None);
}

#[test]
fn an_editor_floats_in_a_window_only_when_its_center_is_inside() {
    let window = rect(0.0, 0.0, 400.0, 300.0);

    assert!(super::editor_floats_in(
        window,
        rect(100.0, 100.0, 120.0, 20.0)
    ));
    assert!(!super::editor_floats_in(
        window,
        rect(500.0, 100.0, 120.0, 20.0)
    ));
    assert!(!super::editor_floats_in(window, None));
    assert!(!super::editor_floats_in(
        None,
        rect(100.0, 100.0, 120.0, 20.0)
    ));
}
