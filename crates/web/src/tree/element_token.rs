use std::fmt;

use agent_desktop_core::{AdapterError, ErrorCode};

const PREFIX: &str = "cdp:";

/// The retained element token of a web node, `cdp:<targetId>:<backendNodeId>`.
/// Snapshots store it in `NodeIdentity.retained_object`, so ref allocation
/// carries it into the ref entry and an action can re-address the DOM node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementToken {
    pub target_id: String,
    pub backend_node_id: i64,
}

impl ElementToken {
    pub fn parse(token: &str) -> Result<Self, AdapterError> {
        let parsed = token.strip_prefix(PREFIX).and_then(|rest| {
            let (target_id, backend) = rest.rsplit_once(':')?;
            let backend_node_id = backend.parse::<i64>().ok().filter(|id| *id > 0)?;
            let valid_target = !target_id.is_empty()
                && target_id
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
            valid_target.then(|| Self {
                target_id: target_id.to_owned(),
                backend_node_id,
            })
        });
        parsed.ok_or_else(|| {
            AdapterError::new(
                ErrorCode::InvalidArgs,
                format!("'{token}' is not a cdp:<targetId>:<backendNodeId> element token"),
            )
        })
    }

    pub fn is_cdp_token(token: &str) -> bool {
        token.starts_with(PREFIX)
    }
}

impl fmt::Display for ElementToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{PREFIX}{}:{}",
            self.target_id, self.backend_node_id
        )
    }
}

#[cfg(test)]
#[path = "element_token_tests.rs"]
mod tests;
