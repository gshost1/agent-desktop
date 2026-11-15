use agent_desktop_core::Deadline;

use super::SessionBus;
use crate::system::bus::SharedBus;
use crate::system::bus::bus_deadline::within;

fn expects(variable: &str) -> bool {
    match std::env::var_os(variable) {
        Some(value) if value == "1" => true,
        Some(value) => panic!("{variable} must be 1 or unset, not {value:?}"),
        None => {
            eprintln!("skipped: {variable} is not set, so this run promises no bus");
            false
        }
    }
}

#[test]
fn the_session_bus_answers_where_the_runner_promises_one() {
    if !expects("AGENT_DESKTOP_EXPECT_SESSION_BUS") {
        return;
    }
    let deadline = Deadline::after(5_000).unwrap();
    let connection = SessionBus::get(deadline).unwrap_or_else(|error| {
        panic!(
            "the runner promised a session bus at {:?}, but it is unreachable: {error:?}",
            std::env::var_os("DBUS_SESSION_BUS_ADDRESS")
        )
    });
    let id: String = within(deadline, async {
        connection
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "GetId",
                &(),
            )
            .await
            .map_err(|error| agent_desktop_core::AdapterError::internal(error.to_string()))?
            .body()
            .deserialize()
            .map_err(|error| agent_desktop_core::AdapterError::internal(error.to_string()))
    })
    .unwrap();
    assert!(!id.is_empty());
}

#[test]
fn the_accessibility_bus_answers_where_the_runner_promises_one() {
    if !expects("AGENT_DESKTOP_EXPECT_ATSPI_BUS") {
        return;
    }
    let deadline = Deadline::after(10_000).unwrap();
    let bus = SharedBus::default();
    if let Err(error) = bus.get(deadline) {
        panic!("the runner promised an AT-SPI bus, but it is unreachable: {error:?}");
    }
}
