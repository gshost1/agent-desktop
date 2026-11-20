use agent_desktop_core::{DisplayInfo, Point, Rect};

use super::{PointerAct, translate};
use crate::input::mouse::pointer_plan::plan_move;
use crate::input::wayland::stream_point::StreamPoint;
use crate::system::DisplayLayout;

fn display(id: &str, bounds: (f64, f64, f64, f64), scale: f64) -> DisplayInfo {
    DisplayInfo {
        id: id.into(),
        bounds: Rect {
            x: bounds.0,
            y: bounds.1,
            width: bounds.2,
            height: bounds.3,
        },
        is_primary: false,
        scale,
    }
}

fn physical_layout() -> DisplayLayout {
    DisplayLayout {
        displays: vec![
            display("Meta-1", (0., 0., 1600., 1200.), 2.),
            display("Meta-0", (1600., 0., 1280., 800.), 1.),
        ],
        stream_scaled: false,
    }
}

fn first_motion(x: f64, y: f64, layout: &DisplayLayout) -> (Vec<String>, StreamPoint) {
    let gesture = translate(
        &plan_move(Point { x, y }).unwrap().steps,
        Vec::new(),
        layout,
    )
    .expect("the point is on a listed display");
    let at = gesture
        .acts
        .iter()
        .find_map(|act| match act {
            PointerAct::Motion(at) => Some(at.clone()),
            _ => None,
        })
        .unwrap();
    (gesture.connectors, at)
}

#[test]
fn a_physical_layout_maps_a_point_one_to_one_on_a_scaled_monitor() {
    let layout = physical_layout();
    assert_eq!(
        first_motion(1000., 500., &layout),
        (
            vec!["Meta-1".to_owned()],
            StreamPoint {
                stream: 0,
                x: 1000.,
                y: 500.
            }
        )
    );
    assert_eq!(
        first_motion(1700., 100., &layout),
        (
            vec!["Meta-0".to_owned()],
            StreamPoint {
                stream: 0,
                x: 100.,
                y: 100.
            }
        )
    );
}

#[test]
fn a_logical_layout_multiplies_the_offset_by_the_monitor_scale() {
    let layout = DisplayLayout {
        displays: vec![
            display("Meta-1", (0., 0., 800., 600.), 2.),
            display("Meta-0", (800., 0., 1280., 800.), 1.),
        ],
        stream_scaled: true,
    };
    assert_eq!(
        first_motion(500., 250., &layout).1,
        StreamPoint {
            stream: 0,
            x: 1000.,
            y: 500.
        }
    );
    assert_eq!(
        first_motion(900., 100., &layout).1,
        StreamPoint {
            stream: 0,
            x: 100.,
            y: 100.
        }
    );
}
