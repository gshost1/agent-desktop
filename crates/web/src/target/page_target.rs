/// A page target as `Target.getTargets` reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTarget {
    pub target_id: String,
    pub url: String,
    pub title: String,
    pub attached: bool,
    pub browser_context_id: Option<String>,
}
