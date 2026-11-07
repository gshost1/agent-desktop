use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RefEntryIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_object: Option<String>,
    pub role: String,
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<crate::ElementIdentifier>,
    /// What the capture this entry was published from proved about its
    /// identity. Read from the legacy `unique_at_capture` flag in a refmap
    /// written before the verdict existed.
    #[serde(
        default,
        alias = "unique_at_capture",
        skip_serializing_if = "crate::CaptureVerdict::is_ambiguous"
    )]
    pub capture: crate::CaptureVerdict,
}
