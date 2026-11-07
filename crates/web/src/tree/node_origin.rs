use serde::{Deserialize, Serialize};

use crate::ElementToken;

/// Where an emitted node lives in the browser: its page target, its frame,
/// and the DOM backend node id CDP actions address it by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeOrigin {
    pub target_id: String,
    pub frame_id: String,
    pub backend_node_id: i64,
}

impl NodeOrigin {
    pub fn element_token(&self) -> ElementToken {
        ElementToken {
            target_id: self.target_id.clone(),
            backend_node_id: self.backend_node_id,
        }
    }
}
