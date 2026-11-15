use agent_desktop_core::{AdapterError, Deadline};

use crate::system::bus::bus_process::owner_process_id;
use crate::system::bus::session_bus::SessionBus;

/// Whether `pid` owns a well-known name on the session bus, as a
/// single-instance application does to receive the launches its later
/// invocations forward. A bus that cannot be read, or does not answer in
/// time, proves no name.
pub(super) fn owns_bus_name(pid: u32, deadline: Deadline) -> bool {
    let Ok(connection) = SessionBus::get(deadline) else {
        return false;
    };
    crate::system::within(deadline, async {
        Ok::<bool, AdapterError>(names_owned_by(pid, &connection).await)
    })
    .unwrap_or(false)
}

async fn names_owned_by(pid: u32, connection: &zbus::Connection) -> bool {
    let Ok(reply) = connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "ListNames",
            &(),
        )
        .await
    else {
        return false;
    };
    let Ok(names) = reply.body().deserialize::<Vec<String>>() else {
        return false;
    };
    for name in names.iter().filter(|name| !name.starts_with(':')) {
        if owner_process_id(connection, name).await.ok() == Some(pid) {
            return true;
        }
    }
    false
}
