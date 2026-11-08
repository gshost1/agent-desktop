/// A flattened CDP session attached to one page target. Requests for the
/// page carry `session_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSession {
    pub target_id: String,
    pub session_id: String,
}
