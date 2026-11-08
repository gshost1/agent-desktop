use super::*;

fn rectangle(x: i32, y: i32, width: i32, height: i32) -> Extents {
    Extents {
        x,
        y,
        width,
        height,
    }
}

fn live(client: Extents, maximized: bool) -> WindowFacts {
    WindowFacts {
        client,
        hidden: false,
        maximized_vertical: maximized,
        maximized_horizontal: maximized,
    }
}

fn two_displays() -> Vec<Rect> {
    vec![
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 800.0,
        },
        Rect {
            x: 1280.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        },
    ]
}

const BEFORE: Extents = Extents {
    x: 120,
    y: 90,
    width: 500,
    height: 380,
};

fn maximized_on_first() -> RememberedPlace {
    RememberedPlace::Maximized {
        before: BEFORE,
        maximized: rectangle(0, 21, 1280, 779),
    }
}

#[test]
fn a_maximized_record_holds_while_the_window_stays_maximized_on_its_display() {
    let settled = live(rectangle(0, 27, 1280, 773), true);
    assert_eq!(
        maximized_on_first().place_for(&settled, &two_displays()),
        Some(BEFORE)
    );
    assert_eq!(maximized_on_first().place_for(&settled, &[]), Some(BEFORE));
}

#[test]
fn a_maximized_record_is_stale_once_the_window_is_unmaximized_or_maximized_elsewhere() {
    let unmaximized = live(rectangle(400, 300, 500, 380), false);
    let elsewhere = live(rectangle(1280, 21, 1920, 1059), true);
    let mut half = live(rectangle(0, 21, 640, 779), true);
    half.maximized_horizontal = false;
    for stale in [unmaximized, elsewhere, half] {
        assert_eq!(
            maximized_on_first().place_for(&stale, &two_displays()),
            None
        );
    }
}

#[test]
fn an_unmaximized_record_holds_only_where_the_agent_left_the_window() {
    let record = RememberedPlace::Unmaximized {
        before: BEFORE,
        left: rectangle(160, 90, 500, 380),
    };
    let displays = two_displays();
    assert_eq!(
        record.place_for(&live(rectangle(161, 91, 500, 380), false), &displays),
        Some(BEFORE)
    );
    assert_eq!(
        record.place_for(&live(rectangle(400, 300, 500, 380), false), &displays),
        None
    );
    assert_eq!(
        record.place_for(&live(rectangle(160, 90, 700, 380), false), &displays),
        None
    );
    assert_eq!(
        record.place_for(&live(rectangle(160, 90, 500, 380), true), &displays),
        None
    );
}

#[test]
fn a_hidden_window_matches_no_record() {
    let mut hidden = live(rectangle(0, 21, 1280, 779), true);
    hidden.hidden = true;
    assert_eq!(maximized_on_first().place_for(&hidden, &[]), None);
}

#[test]
fn a_record_round_trips_and_an_unknown_or_older_shape_reads_as_nothing() {
    let left = RememberedPlace::Unmaximized {
        before: rectangle(-20, 90, 500, 380),
        left: rectangle(160, 90, 500, 380),
    };
    for record in [maximized_on_first(), left] {
        assert_eq!(RememberedPlace::from_json(&record.to_json()), Some(record));
    }
    let mut unknown = left.to_json();
    unknown["phase"] = "moved".into();
    assert_eq!(RememberedPlace::from_json(&unknown), None);
    let older = json!({ "x": 120, "y": 90, "width": 500, "height": 380 });
    assert_eq!(RememberedPlace::from_json(&older), None);
}
