use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

/// Why no pointer can be aimed at a web element: the page is read and
/// driven over CDP, which gives it no screen position, and the web channel
/// never moves the OS cursor.
pub fn no_pointer_position() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "A web element read over CDP has no screen position for the pointer",
    )
    .with_suggestion(
        "Drop --headed: web refs are clicked and typed into through the page, in the background",
    )
    .with_details(serde_json::json!({ "kind": "cdp_no_pointer" }))
    .with_disposition(DeliverySemantics::not_delivered())
}
