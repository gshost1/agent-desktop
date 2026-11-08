use agent_desktop_core::Deadline;

/// How a call may reach the browser: its deadline, and whether it may open an
/// endpoint that asks the user to approve the connection.
#[derive(Debug, Clone, Copy)]
pub struct LinkAccess {
    pub deadline: Deadline,
    pub may_prompt: bool,
}

impl LinkAccess {
    pub fn prompting(deadline: Deadline) -> Self {
        Self {
            deadline,
            may_prompt: true,
        }
    }

    pub fn silent(deadline: Deadline) -> Self {
        Self {
            deadline,
            may_prompt: false,
        }
    }
}
