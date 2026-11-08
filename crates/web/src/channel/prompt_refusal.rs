use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

/// The refusal for an endpoint that would show the user an approval prompt
/// when the caller only reads and may not prompt.
pub(crate) fn approval_needed(port: u16) -> AdapterError {
    AdapterError::new(
        ErrorCode::PermDenied,
        format!("The DevTools endpoint on port {port} asks the user to approve each connection"),
    )
    .with_suggestion(
        "Take a `snapshot --surface web` of the browser to approve the connection once, or \
         relaunch it with `launch <browser> --cdp`",
    )
    .with_details(serde_json::json!({ "kind": "cdp_approval_required", "port": port }))
    .with_disposition(DeliverySemantics::not_delivered())
}
