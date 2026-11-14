//! Synchronous Chrome DevTools Protocol client for agent-desktop.
//!
//! The crate reaches a Chromium browser over a loopback WebSocket, reads each
//! page's accessibility tree into core `AccessibilityNode` trees, and acts on
//! DOM nodes by backend node id without moving the OS cursor or raising the
//! browser window. It depends only on `agent-desktop-core`; the binary wires
//! it beside the platform adapter.

mod act;
mod cdp;
mod channel;
mod discovery;
mod target;
mod transport;
mod tree;

pub use act::assigned_input::AssignedInput;
pub use act::checked::set_checked;
pub use act::click::click;
pub use act::element_reading::ElementReading;
pub use act::fill::fill;
pub use act::fill_outcome::FillOutcome;
pub use act::key_press::press_key;
pub use act::read::read_element;
pub use act::select_option::select_option;
pub use cdp::cdp_event::CdpEvent;
pub use cdp::connection::CdpConnection;
pub use channel::cdp_connector::CdpConnector;
pub use channel::endpoint_candidate::EndpointCandidate;
pub use channel::system_connector::SystemConnector;
pub use channel::web_channel_adapter::WebChannelAdapter;
pub use discovery::browser_endpoint::BrowserEndpoint;
pub use discovery::connect::connect;
pub use discovery::json_version::endpoint_from_port;
pub use discovery::user_data_dir::endpoint_from_user_data_dir;
pub use target::navigate::{navigate, wait_for_load};
pub use target::page_session::PageSession;
pub use target::page_target::PageTarget;
pub use target::targets::{
    attach_to_target, close_target, create_background_tab, list_page_targets,
};
pub use transport::cdp_transport::CdpTransport;
pub use tree::element_token::ElementToken;
pub use tree::node_origin::NodeOrigin;
pub use tree::snapshot::snapshot_page;
pub use tree::web_snapshot::WebSnapshot;

pub(crate) use act::append_text::append_text;
pub(crate) use act::click::click_times;
pub(crate) use act::focus::focus;
pub(crate) use act::scroll::scroll_into_view;
pub(crate) use act::scroll_by::scroll_by;
pub(crate) use agent_desktop_core::DevtoolsWebSocketUrl as WebSocketUrl;
pub(crate) use discovery::active_port::read_active_port;
pub(crate) use discovery::browser_user_data_dir::default_user_data_dirs;
pub(crate) use discovery::endpoint_source::EndpointSource;
pub(crate) use target::browser_window::BrowserWindow;
pub(crate) use target::page_visibility::page_visibility;
pub(crate) use transport::websocket::WebSocketTransport;
pub(crate) use tree::node_read::read_node;
pub(crate) use tree::snapshot::snapshot_page_within_budget;
