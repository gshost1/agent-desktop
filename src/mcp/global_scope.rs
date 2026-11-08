//! Whether a global flag given to `mcp` describes the whole server or a single command.

/// `ServerWide` flags hold for every call the server runs, so the server forwards them to each child.
/// `PerCommand` flags describe one invocation (a create-once visual-debug file, a post-action wait for a
/// selector) and would break every call they do not fit, so `mcp` refuses them before it serves anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlobalScope {
    ServerWide,
    PerCommand,
}

impl GlobalScope {
    /// The scope of the global argument with clap id `id`; `None` for a global this server does not know,
    /// which `mcp` refuses rather than forwards.
    pub(crate) fn of(id: &str) -> Option<Self> {
        match id {
            "verbose" | "session" | "agent_id" | "trace" | "trace_strict" | "headed" => {
                Some(Self::ServerWide)
            }
            "debug" | "screenshot" | "wait_for" | "wait_for_gone" | "wait_timeout" => {
                Some(Self::PerCommand)
            }
            _ => None,
        }
    }
}
