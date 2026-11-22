//! The watch that lets one command reuse an unblocked modal verdict: a
//! private accessibility bus connection subscribed to every object and
//! window event of one application.

use agent_desktop_core::{AdapterError, Deadline};
use futures_lite::future;
use futures_util::StreamExt;
use zbus::connection::Builder;
use zbus::{Connection, MatchRule, MessageStream};

use crate::system::bus::{REGISTRY_ROOT_PATH, classify};
use crate::system::within;
use crate::tree::AtspiElement;
use crate::tree::surfaces::modal_gate::GateWatch;

const QUEUE: usize = 64;
const WATCHED_EVENTS: [&str; 2] = ["object:", "window:"];

/// Holds the subscription open; dropping it closes the connection, which
/// ends the registration with the registry.
pub(crate) struct AppWatch {
    connection: Connection,
    stream: MessageStream,
    app: String,
}

async fn accessibility_bus() -> Result<Connection, AdapterError> {
    let session = Builder::session()
        .map_err(|error| classify(&error))?
        .build()
        .await
        .map_err(|error| classify(&error))?;
    let reply = session
        .call_method(
            Some("org.a11y.Bus"),
            "/org/a11y/bus",
            Some("org.a11y.Bus"),
            "GetAddress",
            &(),
        )
        .await
        .map_err(|error| classify(&error))?;
    let address: String = reply
        .body()
        .deserialize()
        .map_err(|error| classify(&error))?;
    Builder::address(address.as_str())
        .map_err(|error| classify(&error))?
        .build()
        .await
        .map_err(|error| classify(&error))
}

/// Subscribes before registering, so every event the application emits once
/// it learns of the listener lands in the stream. The registry announces a
/// listener before it answers, so a read the application answers after this
/// returns follows the announcement.
pub(crate) fn open(app: &AtspiElement, deadline: Deadline) -> Result<AppWatch, AdapterError> {
    let name = app.bus_name().to_owned();
    within(deadline, async move {
        let connection = accessibility_bus().await?;
        let rule = MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(name.as_str())
            .map_err(|error| classify(&error))?
            .build()
            .to_owned();
        let stream = MessageStream::for_match_rule(rule, &connection, Some(QUEUE))
            .await
            .map_err(|error| classify(&error))?;
        for event in WATCHED_EVENTS {
            connection
                .call_method(
                    Some("org.a11y.atspi.Registry"),
                    "/org/a11y/atspi/registry",
                    Some("org.a11y.atspi.Registry"),
                    "RegisterEvent",
                    &(event, Vec::<String>::new(), name.as_str()),
                )
                .await
                .map_err(|error| classify(&error))?;
        }
        Ok(AppWatch {
            connection,
            stream,
            app: name,
        })
    })
}

impl AppWatch {
    fn answered(&self, deadline: Deadline) -> bool {
        within(deadline, async {
            self.connection
                .call_method(
                    Some(self.app.as_str()),
                    REGISTRY_ROOT_PATH,
                    Some("org.freedesktop.DBus.Peer"),
                    "Ping",
                    &(),
                )
                .await
                .map(|_| ())
                .map_err(|error| classify(&error))
        })
        .is_ok()
    }
}

impl GateWatch for AppWatch {
    /// The application answers a round trip only after the signals it sent
    /// before it, so an empty stream after the answer means no event came.
    fn quiet(&mut self, deadline: Deadline) -> bool {
        if !self.answered(deadline) {
            return false;
        }
        future::block_on(future::poll_once(self.stream.next())).is_none()
    }
}

/// The top-level windows of the application behind `element`, with the
/// process of each one read from its own bus owner.
pub(crate) fn top_levels(
    connection: &Connection,
    element: &AtspiElement,
    deadline: Deadline,
) -> Result<Vec<AtspiElement>, AdapterError> {
    let root = AtspiElement::new(element.bus_name(), REGISTRY_ROOT_PATH, element.pid());
    within(deadline, async {
        let children = crate::tree::list_children(connection, &root).await?;
        let mut windows = Vec::with_capacity(children.len());
        for (bus, path) in children {
            let pid = if bus == element.bus_name() {
                element.pid()
            } else {
                crate::system::process_id(connection, bus.clone()).await.1?
            };
            windows.push(AtspiElement::new(bus, path, pid));
        }
        Ok(windows)
    })
}
