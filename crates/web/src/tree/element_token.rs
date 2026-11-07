use std::fmt;

use agent_desktop_core::{AdapterError, ErrorCode};

const PREFIX: &str = "cdp:";

/// The retained element token of a web node,
/// `cdp:<targetId>:<frameId>:<loaderId>:<backendNodeId>`. The target is the
/// page or out-of-process iframe target whose session addresses the node;
/// the frame and loader name the document the node was read from. Backend
/// node ids restart when a navigation swaps the renderer, so an action
/// first proves the frame still shows that loader's document. Snapshots
/// store the token in `NodeIdentity.retained_object`, so ref allocation
/// carries it into the ref entry and an action can re-address the DOM node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementToken {
    pub target_id: String,
    pub frame_id: String,
    pub loader_id: String,
    pub backend_node_id: i64,
}

impl ElementToken {
    /// Parses the five-part form. The earlier `cdp:<targetId>:<backendNodeId>`
    /// form carries no document identity, so it is `STALE_REF`: only a new
    /// snapshot can say which document its node belonged to.
    pub fn parse(token: &str) -> Result<Self, AdapterError> {
        let Some(rest) = token.strip_prefix(PREFIX) else {
            return Err(malformed(token));
        };
        let parts: Vec<&str> = rest.split(':').collect();
        let backend_node_id = parts
            .last()
            .and_then(|backend| backend.parse::<i64>().ok())
            .filter(|id| *id > 0);
        match (parts.as_slice(), backend_node_id) {
            ([target_id, frame_id, loader_id, _], Some(backend_node_id))
                if [target_id, frame_id, loader_id]
                    .iter()
                    .all(|part| is_protocol_id(part)) =>
            {
                Ok(Self {
                    target_id: (*target_id).to_owned(),
                    frame_id: (*frame_id).to_owned(),
                    loader_id: (*loader_id).to_owned(),
                    backend_node_id,
                })
            }
            ([target_id, _], Some(_)) if is_protocol_id(target_id) => Err(legacy(token)),
            _ => Err(malformed(token)),
        }
    }

    pub fn is_cdp_token(token: &str) -> bool {
        token.starts_with(PREFIX)
    }
}

fn is_protocol_id(part: &str) -> bool {
    !part.is_empty()
        && part
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
}

fn malformed(token: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        format!(
            "'{token}' is not a cdp:<targetId>:<frameId>:<loaderId>:<backendNodeId> element token"
        ),
    )
}

fn legacy(token: &str) -> AdapterError {
    AdapterError::stale_ref_because(format!("'{token}' predates document-scoped element tokens"))
        .with_suggestion("Take a new snapshot and use the ref it returns")
        .with_details(serde_json::json!({ "kind": "cdp_token_legacy" }))
}

impl fmt::Display for ElementToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{PREFIX}{}:{}:{}:{}",
            self.target_id, self.frame_id, self.loader_id, self.backend_node_id
        )
    }
}

#[cfg(test)]
#[path = "element_token_tests.rs"]
mod tests;
