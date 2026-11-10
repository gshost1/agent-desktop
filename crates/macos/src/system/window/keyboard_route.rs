use super::hosted_focus::Candidate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyboardRoute {
    Host,
    HostedView { pid: i32 },
    Unproven { pid: i32, reason: &'static str },
}

impl KeyboardRoute {
    pub(crate) fn hosted_pid(self) -> Option<i32> {
        match self {
            Self::HostedView { pid } => Some(pid),
            Self::Host | Self::Unproven { .. } => None,
        }
    }

    /// The `keyboard_target` report: which pid received the key and, when the
    /// focus owner could not be proven, why, with every candidate's evidence.
    pub(crate) fn details(self, host: i32, candidates: &[Candidate]) -> Option<serde_json::Value> {
        let evidence = || {
            candidates
                .iter()
                .map(Candidate::evidence)
                .collect::<Vec<_>>()
        };
        match self {
            Self::Host => None,
            Self::HostedView { pid } => Some(serde_json::json!({
                "pid": pid,
                "host_pid": host,
                "route": "hosted_view",
                "candidates": evidence(),
            })),
            Self::Unproven { pid, reason } => Some(serde_json::json!({
                "pid": host,
                "host_pid": host,
                "route": "host",
                "focus_owner_pid": pid,
                "unproven": reason,
                "candidates": evidence(),
            })),
        }
    }
}
