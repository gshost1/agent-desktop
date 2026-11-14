//! What the daemon answers. A client may run the command itself after `Declined` or `Rerun`,
//! never after `Accepted` unless the command only reads.

/// One message from the daemon to a client.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum DaemonReply {
    /// Nothing ran; the client runs the command in its own process.
    Declined { reason: String },
    /// The command is starting. `rerun_safe` holds when it only reads, so a daemon that dies
    /// before it finishes leaves nothing behind that a second run could repeat.
    Accepted { command: String, rerun_safe: bool },
    /// The command finished; its stdout and stderr bytes follow as two frames.
    Done { exit: u8 },
    /// The command was refused before dispatch for a reason this process may not share, such
    /// as a permission the daemon lacks; nothing was delivered and the client runs it itself.
    Rerun { reason: String },
}
