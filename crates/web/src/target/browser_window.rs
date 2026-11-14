use agent_desktop_core::{AdapterError, Deadline, Rect};
use serde_json::{Value, json};

use crate::CdpConnection;
use crate::transport::transport_error::transport_failed;

/// The browser window that holds a page target, as `Browser.getWindowForTarget`
/// reports it: the browser's window id and its screen bounds in DIPs.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BrowserWindow {
    pub(crate) window_id: i64,
    pub(crate) bounds: Rect,
}

impl BrowserWindow {
    /// The window that shows `target_id`. Reading it raises nothing.
    pub(crate) fn of_target(
        connection: &mut CdpConnection,
        target_id: &str,
        deadline: Deadline,
    ) -> Result<Self, AdapterError> {
        let reply = connection.call(
            "Browser.getWindowForTarget",
            json!({ "targetId": target_id }),
            None,
            deadline,
        )?;
        let number = |key: &str| {
            reply
                .pointer(&format!("/bounds/{key}"))
                .and_then(Value::as_f64)
        };
        let window_id = reply.get("windowId").and_then(Value::as_i64);
        match (
            window_id,
            number("left"),
            number("top"),
            number("width"),
            number("height"),
        ) {
            (Some(window_id), Some(x), Some(y), Some(width), Some(height)) => Ok(Self {
                window_id,
                bounds: Rect {
                    x,
                    y,
                    width,
                    height,
                },
            }),
            _ => Err(transport_failed(
                "Browser.getWindowForTarget",
                "reply has no window id and bounds",
            )),
        }
    }
}

#[cfg(test)]
#[path = "browser_window_tests.rs"]
mod tests;
