use super::choose_focused_root;
use agent_desktop_core::{AdapterError, ErrorCode, Rect};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Option<Rect> {
    Some(Rect {
        x,
        y,
        width,
        height,
    })
}

const WINDOW: Option<Rect> = Some(Rect {
    x: 100.0,
    y: 100.0,
    width: 800.0,
    height: 500.0,
});

#[test]
fn an_editor_floating_in_the_focused_window_is_the_focused_surface() {
    let root = choose_focused_root(Some(("window", WINDOW)), || {
        Ok(Some(("editor", rect(300.0, 240.0, 140.0, 20.0))))
    })
    .unwrap();

    assert_eq!(root, Some("editor"));
}

#[test]
fn a_focused_editor_is_the_focused_surface_while_no_window_reports_focus() {
    let root = choose_focused_root(None, || Ok(Some(("editor", None)))).unwrap();

    assert_eq!(root, Some("editor"));
}

#[test]
fn the_window_stays_the_surface_without_proof_the_editor_is_inside_it() {
    for editor in [
        Ok(None),
        Ok(Some(("editor", None))),
        Ok(Some(("editor", rect(1200.0, 240.0, 140.0, 20.0)))),
        Err(AdapterError::new(ErrorCode::Timeout, "slow")),
    ] {
        let root = choose_focused_root(Some(("window", WINDOW)), || editor).unwrap();
        assert_eq!(root, Some("window"));
    }
    let root = choose_focused_root(Some(("window", None)), || {
        Ok(Some(("editor", rect(300.0, 240.0, 140.0, 20.0))))
    })
    .unwrap();
    assert_eq!(root, Some("window"));
}

#[test]
fn an_editor_read_failure_without_a_focused_window_is_reported() {
    let error = choose_focused_root::<&str>(None, || {
        Err(AdapterError::new(ErrorCode::PermDenied, "denied"))
    })
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(
        choose_focused_root::<&str>(None, || Ok(None)).unwrap(),
        None
    );
}
