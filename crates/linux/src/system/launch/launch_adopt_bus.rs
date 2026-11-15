use agent_desktop_core::{AdapterError, Deadline};

use crate::system::bus::session_bus::SessionBus;

/// The interfaces a single-instance application exports on its application
/// object to receive the launches its later invocations forward.
const LAUNCH_INTERFACES: [&str; 2] = ["org.freedesktop.Application", "org.gtk.Application"];

/// Whether `pid` receives forwarded launches over the session bus: it owns
/// the name of the desktop entry the launch started, or a well-known name
/// whose application object implements a launch interface. Any other name it
/// owns, such as a media player or tray name, proves nothing. A bus that
/// cannot be read, or does not answer in time, proves nothing either.
pub(super) fn serves_launches(pid: u32, desktop_id: Option<&str>, deadline: Deadline) -> bool {
    let Ok(connection) = SessionBus::get(deadline) else {
        return false;
    };
    crate::system::within(deadline, async {
        Ok::<bool, AdapterError>(serving(pid, desktop_id, &connection).await)
    })
    .unwrap_or(false)
}

async fn serving(pid: u32, desktop_id: Option<&str>, connection: &zbus::Connection) -> bool {
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
        let owner =
            call::<_, u32>(connection, DAEMON, "GetConnectionUnixProcessID", &(name,)).await;
        if owner != Some(pid) {
            continue;
        }
        if desktop_id == Some(name.as_str()) || exports_launch_interface(connection, name).await {
            return true;
        }
    }
    false
}

async fn exports_launch_interface(connection: &zbus::Connection, name: &str) -> bool {
    let path = application_path(name);
    let target = [name, path.as_str(), "org.freedesktop.DBus.Introspectable"];
    call::<_, String>(connection, target, "Introspect", &())
        .await
        .is_some_and(|xml| declares_launch_interface(&xml))
}

const DAEMON: [&str; 3] = [
    "org.freedesktop.DBus",
    "/org/freedesktop/DBus",
    "org.freedesktop.DBus",
];

/// One method call to `[destination, path, interface]` that never starts a
/// service the bus could activate for the name, and its reply body.
async fn call<B, R>(
    connection: &zbus::Connection,
    [destination, path, interface]: [&str; 3],
    method: &str,
    body: &B,
) -> Option<R>
where
    B: zbus::export::serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let proxy = zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
        .destination(destination)
        .ok()?
        .path(path)
        .ok()?
        .interface(interface)
        .ok()?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .ok()?;
    proxy
        .call_with_flags::<_, B, R>(method, zbus::proxy::MethodFlags::NoAutoStart.into(), body)
        .await
        .ok()
        .flatten()
}

/// The object path an application exports under its bus name, as GLib and
/// the desktop entry specification derive it: dots become slashes and
/// dashes underscores.
pub(super) fn application_path(name: &str) -> String {
    format!("/{}", name.replace('.', "/").replace('-', "_"))
}

pub(super) fn declares_launch_interface(xml: &str) -> bool {
    LAUNCH_INTERFACES
        .iter()
        .any(|interface| xml.contains(&format!("<interface name=\"{interface}\"")))
}
