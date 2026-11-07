use serde::{Deserialize, Serialize};

use crate::ElementToken;

/// Where an emitted node lives in the browser: the page or iframe target
/// whose session reads it, its frame, the loader of the document it was
/// read from, and the DOM backend node id CDP actions address it by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeOrigin {
    pub target_id: String,
    pub frame_id: String,
    pub loader_id: String,
    pub backend_node_id: i64,
}

impl NodeOrigin {
    pub fn element_token(&self) -> ElementToken {
        ElementToken {
            target_id: self.target_id.clone(),
            frame_id: self.frame_id.clone(),
            loader_id: self.loader_id.clone(),
            backend_node_id: self.backend_node_id,
        }
    }
}
