use agent_desktop_core::Point;
use serde_json::json;

use super::ViewportMetrics;
use crate::CdpConnection;
use crate::act::act_fixture::{deadline, session};
use crate::transport::scripted_transport::ScriptedTransport;

fn scrolled() -> ViewportMetrics {
    ViewportMetrics {
        page_x: 0.0,
        page_y: 1_800.0,
        width: 800.0,
        height: 600.0,
        zoom: 1.0,
    }
}

#[test]
fn a_scrolled_viewport_point_is_offset_into_the_document() {
    let hit = scrolled().hit_point(Point { x: 45.0, y: 583.0 });

    assert_eq!(hit.viewport, Point { x: 45.0, y: 583.0 });
    assert_eq!(
        hit.document,
        Point {
            x: 45.0,
            y: 2_383.0
        }
    );
}

#[test]
fn a_pinch_zoomed_viewport_point_is_scaled_back_to_css_pixels() {
    let zoomed = ViewportMetrics {
        page_x: 100.0,
        page_y: 50.0,
        zoom: 2.0,
        ..scrolled()
    };

    let hit = zoomed.hit_point(Point { x: 40.0, y: 20.0 });

    assert_eq!(hit.document, Point { x: 120.0, y: 60.0 });
}

#[test]
fn only_the_visible_part_of_a_quad_gives_its_centre() {
    let metrics = scrolled();

    assert_eq!(
        metrics.visible_center(&json!([700, 500, 900, 500, 900, 700, 700, 700])),
        Some(Point { x: 750.0, y: 550.0 })
    );
    assert_eq!(
        metrics.visible_center(&json!([0, 650, 10, 650, 10, 660, 0, 660])),
        None
    );
    assert_eq!(metrics.visible_center(&json!([1, 2, 3])), None);
}

#[test]
fn the_metrics_come_from_the_css_visual_viewport() {
    let transport = ScriptedTransport::new().expect(
        "Page.getLayoutMetrics",
        json!({
            "cssLayoutViewport": { "pageX": 0, "pageY": 1_800, "clientWidth": 800, "clientHeight": 600 },
            "cssVisualViewport": { "pageX": 0, "pageY": 1_800, "clientWidth": 800, "clientHeight": 600, "zoom": 1 }
        }),
    );
    let sent = transport.sent_log();
    let mut connection = CdpConnection::new(transport.boxed());

    let metrics = ViewportMetrics::read(&mut connection, &session(), deadline()).unwrap();

    assert_eq!(metrics, scrolled());
    assert_eq!(sent.lock().unwrap()[0]["sessionId"], "S");
}
