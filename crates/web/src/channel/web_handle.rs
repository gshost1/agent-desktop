use agent_desktop_core::{NativeHandle, ProcessIdentity};

use crate::ElementToken;

/// The payload of a `NativeHandle` that names a DOM node: the browser process
/// whose DevTools endpoint reaches it, and its element token. Every adapter
/// method that takes a handle checks for this payload first, so a web handle
/// is answered over CDP and never reaches the platform adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebHandle {
    pub process: ProcessIdentity,
    pub token: ElementToken,
}

impl WebHandle {
    /// The web payload of `handle`, if it carries one.
    pub fn of(handle: &NativeHandle) -> Option<&Self> {
        handle.downcast_ref::<Self>()
    }
}
