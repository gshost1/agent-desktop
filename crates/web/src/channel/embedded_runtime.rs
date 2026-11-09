//! Apps that embed a browser runtime for their own interface. Their pages are
//! the app itself, so `open-url` never replaces one.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use super::browser_link::BrowserLink;

/// The token an embedded browser runtime adds to its user agent.
const EMBEDDED_RUNTIME_TOKEN: &str = " Electron/";

/// Refuses `open-url` when the endpoint belongs to an app that embeds the
/// browser runtime, read from the runtime's own user agent.
pub(crate) fn refuse_embedded_runtime(
    link: &mut BrowserLink,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let version = link
        .connection
        .call("Browser.getVersion", json!({}), None, deadline)?;
    let user_agent = version
        .get("userAgent")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if is_embedded_runtime(user_agent) {
        return Err(not_a_browser());
    }
    Ok(())
}

pub(crate) fn is_embedded_runtime(user_agent: &str) -> bool {
    user_agent.contains(EMBEDDED_RUNTIME_TOKEN)
}

fn not_a_browser() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "This app embeds a browser runtime for its own interface; open-url loads pages only in a web browser",
    )
    .with_suggestion(
        "Read the app's web content with snapshot --surface web -i and act on its refs; open URLs in a web browser",
    )
    .with_details(json!({ "kind": "cdp_not_a_browser" }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "embedded_runtime_tests.rs"]
mod tests;
