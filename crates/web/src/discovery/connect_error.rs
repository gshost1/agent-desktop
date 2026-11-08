use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

pub(crate) fn endpoint_unreachable(port: u16, detail: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("No DevTools server is listening on 127.0.0.1:{port}"),
    )
    .with_suggestion(
        "Start the browser with remote debugging enabled; a DevToolsActivePort file left by \
         an exited browser names a port nothing listens on",
    )
    .with_platform_detail(detail)
    .with_details(serde_json::json!({ "kind": "cdp_endpoint_unavailable", "port": port }))
    .with_disposition(DeliverySemantics::not_delivered())
}

pub(crate) fn handshake_rejected(status: u16, port: u16) -> AdapterError {
    AdapterError::new(
        ErrorCode::PermDenied,
        format!("The browser refused the DevTools connection on port {port} (HTTP {status})"),
    )
    .with_suggestion(
        "Allow the remote-debugging connection in the browser, or enable remote debugging \
         at chrome://inspect/#remote-debugging, then retry",
    )
    .with_details(serde_json::json!({
        "kind": "cdp_connection_rejected",
        "port": port,
        "http_status": status,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

pub(crate) fn approval_pending(port: u16, elapsed_ms: u128) -> AdapterError {
    AdapterError::new(
        ErrorCode::PermDenied,
        format!(
            "The browser on port {port} did not approve the DevTools connection before the deadline"
        ),
    )
    .with_suggestion(
        "Accept the browser's 'Allow remote debugging' prompt, then retry with a longer timeout",
    )
    .with_details(serde_json::json!({
        "kind": "cdp_approval_pending",
        "port": port,
        "elapsed_ms": elapsed_ms,
        "retryable": true,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

pub(crate) fn connection_declined(port: u16, detail: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::PermDenied,
        format!("The browser on port {port} closed the DevTools connection before it was approved"),
    )
    .with_suggestion("The remote-debugging prompt was likely declined; accept it and retry")
    .with_platform_detail(detail)
    .with_details(serde_json::json!({ "kind": "cdp_connection_rejected", "port": port }))
    .with_disposition(DeliverySemantics::not_delivered())
}
