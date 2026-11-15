//! Which pointer gestures a Wayland session can deliver. The compositor places
//! input at a point through Mutter's RemoteDesktop session and a ScreenCast
//! stream of the monitor, but no client can learn which window is under that
//! point. A gesture addressed by coordinates is therefore deliverable and
//! unverified, and a gesture aimed at an element stays refused.

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::json;
use zbus::Connection;

use super::remote_desktop::{RemoteDesktop, semantic_action_refusal};
use crate::input::backend::Backend;
use crate::system::bus::session_bus::{SessionBus, is_transport_failure};
use crate::system::within;

const PROBE_BUDGET_MS: u64 = 2_000;
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const REMOTE_DESKTOP: (&str, &str) = (
    "org.gnome.Mutter.RemoteDesktop",
    "/org/gnome/Mutter/RemoteDesktop",
);
const SCREEN_CAST: (&str, &str) = (
    "org.gnome.Mutter.ScreenCast",
    "/org/gnome/Mutter/ScreenCast",
);

/// The refusal for a pointer gesture aimed at an element on Wayland.
pub(crate) fn element_pointer_refusal() -> AdapterError {
    semantic_action_refusal(
        "Pointer input aimed at an element is not available on Wayland: the compositor does not tell clients which window is under a point, so the target cannot be proven",
    )
    .with_suggestion(
        "Use the element's semantic action (the ref command without --headed), or send the pointer to screen coordinates with mouse-click, mouse-move, mouse-wheel, hover --xy or drag --from-xy/--to-xy, which Wayland delivers unverified",
    )
    .with_details(json!({
        "cause": "wayland_session",
        "physical_delivery_started": false,
    }))
}

fn missing_interface(service: &'static str, detail: String) -> AdapterError {
    AdapterError::new(
        ErrorCode::PlatformNotSupported,
        format!(
            "Pointer input at coordinates on Wayland needs the compositor interface {service}, which this session does not provide"
        ),
    )
    .with_suggestion(
        "Use the element's semantic action instead; physical pointer input on Wayland is available only where the compositor offers RemoteDesktop and ScreenCast",
    )
    .with_platform_detail(detail)
    .with_details(json!({
        "cause": "wayland_interface_missing",
        "missing_interface": service,
        "physical_delivery_started": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

async fn answers(
    connection: &Connection,
    (service, path): (&'static str, &'static str),
) -> Result<(), AdapterError> {
    match connection
        .call_method(
            Some(service),
            path,
            Some(PROPERTIES),
            "Get",
            &(service, "Version"),
        )
        .await
    {
        Ok(_) => Ok(()),
        Err(error) if is_transport_failure(&error) => {
            SessionBus::forget(connection);
            Err(
                AdapterError::new(ErrorCode::ActionFailed, "The session bus failed")
                    .with_platform_detail(error.to_string())
                    .with_disposition(DeliverySemantics::not_delivered()),
            )
        }
        Err(error) => Err(missing_interface(service, error.to_string())),
    }
}

/// Checks that both compositor interfaces a coordinate gesture needs answer
/// on this session bus.
pub(crate) fn probe(connection: &Connection, deadline: Deadline) -> Result<(), AdapterError> {
    within(deadline, async {
        answers(connection, REMOTE_DESKTOP).await?;
        answers(connection, SCREEN_CAST).await
    })
}

fn refused_unsent(mut error: AdapterError) -> AdapterError {
    let mut details = match error.details.take() {
        Some(serde_json::Value::Object(map)) => map,
        Some(other) => serde_json::Map::from_iter([("detail".to_owned(), other)]),
        None => serde_json::Map::new(),
    };
    details.insert("physical_delivery_started".to_owned(), false.into());
    error.details = Some(serde_json::Value::Object(details));
    error.with_disposition(DeliverySemantics::not_delivered())
}

/// Decides the element-aimed gate from the session's input route.
pub(crate) fn element_gate(route: Result<Backend, AdapterError>) -> Option<AdapterError> {
    match route {
        Ok(Backend::Wayland) => Some(element_pointer_refusal()),
        _ => None,
    }
}

/// Decides the coordinate gate from the session's input route and, on
/// Wayland only, the probe of the compositor interfaces.
pub(crate) fn coordinate_gate(
    route: Result<Backend, AdapterError>,
    probe: impl FnOnce() -> Result<(), AdapterError>,
) -> Option<AdapterError> {
    match route {
        Ok(Backend::Wayland) => probe().err().map(refused_unsent),
        _ => None,
    }
}

/// Opens the compositor input session for a pointer gesture. A session bus
/// where RemoteDesktop or ScreenCast does not answer refuses with the missing
/// interface, so every caller of the adapter gets the gate's answer.
pub(crate) fn open_input_session(deadline: Deadline) -> Result<RemoteDesktop, AdapterError> {
    let connection = SessionBus::get(deadline)?;
    probe(&connection, deadline).map_err(refused_unsent)?;
    Ok(RemoteDesktop::over(connection))
}

/// Probes this session's bus for the interfaces a coordinate gesture needs.
pub(crate) fn probe_session() -> Result<(), AdapterError> {
    open_input_session(Deadline::after(PROBE_BUDGET_MS)?).map(|_| ())
}

#[cfg(test)]
#[path = "pointer_gate_tests.rs"]
mod tests;
