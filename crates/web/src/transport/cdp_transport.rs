use agent_desktop_core::{AdapterError, Deadline};

/// A message channel that carries CDP JSON text frames. Every read and write
/// is bounded by the caller's deadline; an expired deadline is a `TIMEOUT`
/// error, never a block.
pub trait CdpTransport: Send {
    fn send_text(&mut self, text: &str, deadline: Deadline) -> Result<(), AdapterError>;

    fn receive_text(&mut self, deadline: Deadline) -> Result<String, AdapterError>;
}
