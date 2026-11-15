//! What the daemon answers, in order: `Declined`, or `Ready`; after the client's go, `Started`,
//! `Running` heartbeats while the command runs, then `Done` or `Rerun`. The client may run the
//! command itself whenever `Started` has not arrived, and after it only when the command reads.

/// The client's one message after `Ready`: run it. Without it the daemon never runs the command.
pub(crate) const GO: &str = "go";

/// One message from the daemon to a client.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum DaemonReply {
    /// Nothing ran; the client runs the command in its own process.
    Declined { reason: String },
    /// The daemon holds its execution slot for this command and waits for the client's go.
    /// `rerun_safe` holds when the command only reads, so a daemon that dies before it finishes
    /// leaves nothing behind that a second run could repeat.
    Ready { command: String, rerun_safe: bool },
    /// The command is running from here on.
    Started,
    /// The daemon is alive and the command still runs.
    Running,
    /// The command finished; its stdout and stderr bytes follow as two frames.
    Done { exit: u8 },
    /// The command was refused before dispatch for a reason this process may not share, such
    /// as a permission the daemon lacks; nothing was delivered and the client runs it itself.
    Rerun { reason: String },
}
