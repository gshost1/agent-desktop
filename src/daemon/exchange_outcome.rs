//! How a client's exchange with its daemon ended after it said go.

/// How an exchange after the go ended.
pub(crate) enum Outcome {
    Finished {
        exit: u8,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// Nothing ran in the daemon: it stopped before `Started` or asked for a rerun.
    NotRun,
    /// The daemon started the command and then stopped answering.
    Lost,
}
