use agent_desktop_core::{Deadline, Rect};
use serde_json::json;

use super::BrowserWindow;
use crate::CdpConnection;
use crate::transport::scripted_transport::ScriptedTransport;

#[test]
fn the_window_of_a_target_carries_its_id_and_bounds() {
    let transport = ScriptedTransport::new().expect(
        "Browser.getWindowForTarget",
        json!({ "windowId": 3, "bounds": { "left": 10, "top": 20, "width": 800, "height": 600, "windowState": "normal" } }),
    );
    let mut connection = CdpConnection::new(transport.boxed());
    let window =
        BrowserWindow::of_target(&mut connection, "T", Deadline::after(500).unwrap()).unwrap();
    assert_eq!(window.window_id, 3);
    let ax = Rect {
        x: 11.0,
        y: 20.0,
        width: 800.0,
        height: 599.0,
    };
    assert!(window.matches_bounds(&ax, 2.0));
    let elsewhere = Rect { x: 400.0, ..ax };
    assert!(!window.matches_bounds(&elsewhere, 2.0));
}

#[test]
fn a_reply_without_bounds_is_an_error() {
    let transport =
        ScriptedTransport::new().expect("Browser.getWindowForTarget", json!({ "windowId": 3 }));
    let mut connection = CdpConnection::new(transport.boxed());
    assert!(BrowserWindow::of_target(&mut connection, "T", Deadline::after(500).unwrap()).is_err());
}
