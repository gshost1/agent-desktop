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
            "The browser on port {port} is waiting for its user to allow the DevTools connection"
        ),
    )
    .with_suggestion(
        "The browser shows an 'Allow remote debugging?' prompt on its window, and every new \
         connection adds one more, so do not retry yet. Read it with `agent-desktop \
         list-surfaces --app <browser>` and `agent-desktop snapshot --app <browser> --surface \
         sheet -i` (on macOS it is a sheet; elsewhere snapshot the dialog window), click \
         Allow only when the user asked to use this browser and Cancel otherwise, then retry once",
    )
    .with_details(serde_json::json!({
        "kind": "cdp_approval_pending",
        "port": port,
        "elapsed_ms": elapsed_ms,
        "retryable": false,
        "approval_surface": "sheet",
    }))
    .with_disposition(DeliverySemantics::uncertain())
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
