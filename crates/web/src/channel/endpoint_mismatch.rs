use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

/// The refusal for an endpoint whose WebSocket URL names a port other than
/// the one the browser process was checked to own. Connecting there would
/// talk to whatever listens on that port instead of the browser.
pub(crate) fn endpoint_mismatch(checked: u16, advertised: u16) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "The DevTools endpoint for port {checked} points at port {advertised}, which the \
             browser was not checked to own"
        ),
    )
    .with_suggestion(
        "Relaunch the browser with `launch <browser> --cdp` to record a fresh endpoint",
    )
    .with_details(serde_json::json!({
        "kind": "cdp_endpoint_mismatch",
        "port": checked,
        "advertised_port": advertised,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}
