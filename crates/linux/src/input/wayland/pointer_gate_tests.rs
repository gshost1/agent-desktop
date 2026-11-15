use std::cell::Cell;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use zbus::connection::Builder;

use super::{coordinate_gate, element_gate, missing_interface, open_input_session, probe};
use crate::input::backend::Backend;
use crate::system::bus::session_bus::SessionBus;

fn x11() -> Result<Backend, AdapterError> {
    Ok(Backend::X11(":151".into()))
}

#[test]
fn wayland_refuses_element_aimed_pointer_input_and_names_the_coordinate_route() {
    let error = element_gate(Ok(Backend::Wayland)).expect("Wayland cannot prove a window");
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    let details = error.details.expect("details");
    assert_eq!(details["cause"], "wayland_session");
    assert_eq!(details["physical_delivery_started"], false);
    let suggestion = error.suggestion.expect("a next step");
    assert!(suggestion.contains("mouse-click") && suggestion.contains("--xy"));
    assert!(element_gate(x11()).is_none());
}

#[test]
fn wayland_opens_the_coordinate_gate_only_when_the_interfaces_answer() {
    let probes = Cell::new(0);
    let probe_ok = || {
        probes.set(probes.get() + 1);
        Ok(())
    };
    assert!(coordinate_gate(Ok(Backend::Wayland), probe_ok).is_none());
    assert_eq!(probes.get(), 1);
    assert!(
        coordinate_gate(x11(), || -> Result<(), AdapterError> {
            panic!("X11 never probes the compositor")
        })
        .is_none()
    );
    let error = coordinate_gate(Ok(Backend::Wayland), || {
        Err(missing_interface(
            "org.gnome.Mutter.ScreenCast",
            "ServiceUnknown".into(),
        ))
    })
    .expect("a missing interface refuses coordinate input");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.message.contains("org.gnome.Mutter.ScreenCast"));
    let details = error.details.expect("details");
    assert_eq!(details["missing_interface"], "org.gnome.Mutter.ScreenCast");
    assert_eq!(details["physical_delivery_started"], false);
}

struct Versioned;

#[zbus::interface(name = "org.gnome.Mutter.RemoteDesktop")]
impl Versioned {
    #[zbus(property)]
    fn version(&self) -> i32 {
        1
    }
}

struct CastVersioned;

#[zbus::interface(name = "org.gnome.Mutter.ScreenCast")]
impl CastVersioned {
    #[zbus(property)]
    fn version(&self) -> i32 {
        4
    }
}

fn missing_after_probe() -> Option<String> {
    let deadline = Deadline::after(5_000).unwrap();
    let connection = SessionBus::get(deadline).unwrap();
    probe(&connection, deadline).err().map(|error| {
        error.details.unwrap()["missing_interface"]
            .as_str()
            .unwrap()
            .to_owned()
    })
}

#[test]
#[ignore = "requires scripts/private-session.sh without a compositor service"]
fn the_probe_names_the_first_missing_compositor_interface() {
    assert!(std::env::var_os("AGENT_DESKTOP_PRIVATE_SESSION").is_some());
    assert_eq!(
        missing_after_probe().as_deref(),
        Some("org.gnome.Mutter.RemoteDesktop")
    );
    let error = open_input_session(Deadline::after(5_000).unwrap())
        .err()
        .expect("the delivery path refuses before any session exists");
    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(error.details.unwrap()["cause"], "wayland_interface_missing");
    let remote = zbus::block_on(async {
        Builder::session()
            .unwrap()
            .name("org.gnome.Mutter.RemoteDesktop")
            .unwrap()
            .serve_at("/org/gnome/Mutter/RemoteDesktop", Versioned)
            .unwrap()
            .build()
            .await
            .unwrap()
    });
    assert_eq!(
        missing_after_probe().as_deref(),
        Some("org.gnome.Mutter.ScreenCast")
    );
    let cast = zbus::block_on(async {
        Builder::session()
            .unwrap()
            .name("org.gnome.Mutter.ScreenCast")
            .unwrap()
            .serve_at("/org/gnome/Mutter/ScreenCast", CastVersioned)
            .unwrap()
            .build()
            .await
            .unwrap()
    });
    assert_eq!(missing_after_probe(), None);
    drop((remote, cast));
}

#[test]
fn a_probe_refusal_with_non_object_details_is_stamped_unsent_without_panicking() {
    let refusal = coordinate_gate(Ok(Backend::Wayland), || {
        Err(AdapterError::new(ErrorCode::ActionFailed, "probe failed")
            .with_details(serde_json::json!("a string, not an object")))
    })
    .expect("the probe refused");
    let details = refusal.details.expect("details");
    assert_eq!(details["physical_delivery_started"], false);
    assert_eq!(details["detail"], "a string, not an object");
    assert_eq!(refusal.disposition, DeliverySemantics::not_delivered());
}
