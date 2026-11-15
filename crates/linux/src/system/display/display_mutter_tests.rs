use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use super::{Mode, Physical, Properties, Reply, Spec, assemble};

fn spec(connector: &str) -> Spec {
    (
        connector.into(),
        "MetaVendor".into(),
        "MetaProduct".into(),
        "0x1".into(),
    )
}

fn current(width: i32, height: i32) -> Mode {
    let properties: Properties = HashMap::from([("is-current".to_owned(), OwnedValue::from(true))]);
    (
        format!("{width}x{height}"),
        width,
        height,
        60.0,
        1.0,
        vec![1.0, 2.0],
        properties,
    )
}

fn reply(layout_mode: u32) -> Reply {
    let monitors: Vec<Physical> = vec![
        (spec("Meta-1"), vec![current(1600, 1200)], HashMap::new()),
        (spec("Meta-0"), vec![current(1280, 800)], HashMap::new()),
    ];
    let left_width = if layout_mode == 1 { 800 } else { 1600 };
    let logical = vec![
        (0, 0, 2.0, 0, true, vec![spec("Meta-1")], HashMap::new()),
        (
            left_width,
            0,
            1.0,
            0,
            false,
            vec![spec("Meta-0")],
            HashMap::new(),
        ),
    ];
    let properties: Properties =
        HashMap::from([("layout-mode".to_owned(), OwnedValue::from(layout_mode))]);
    (1, monitors, logical, properties)
}

#[test]
fn a_logical_layout_divides_the_mode_by_the_scale_and_says_so() {
    let state = assemble(reply(1));
    assert!(state.layout_is_logical);
    let sizes: Vec<_> = state
        .monitors
        .iter()
        .map(|monitor| (monitor.x, monitor.width, monitor.height))
        .collect();
    assert_eq!(sizes, [(0, 800, 600), (800, 1280, 800)]);
}

#[test]
fn a_physical_layout_keeps_the_mode_size_and_says_so() {
    let state = assemble(reply(2));
    assert!(!state.layout_is_logical);
    let sizes: Vec<_> = state
        .monitors
        .iter()
        .map(|monitor| (monitor.x, monitor.width, monitor.height))
        .collect();
    assert_eq!(sizes, [(0, 1600, 1200), (1600, 1280, 800)]);
}

#[test]
fn a_reply_without_a_layout_mode_is_read_as_physical() {
    let (serial, monitors, logical, _) = reply(1);
    let state = assemble((serial, monitors, logical, HashMap::new()));
    assert!(!state.layout_is_logical);
    assert_eq!(state.monitors[0].width, 1600);
}
