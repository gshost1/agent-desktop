use serde::Serialize;

/// The identity a ref is re-identified by. It deserializes by hand
/// (`ref_entry_identity_serde.rs`) so the capture verdict reads strictly from
/// `capture` and the boolean of an older refmap only from `unique_at_capture`.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct RefEntryIdentity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retained_object: Option<String>,
    pub role: String,
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_id: Option<crate::ElementIdentifier>,
    /// What the capture this entry was published from proved about its
    /// identity. An entry no capture judged is `Unknown`; a written entry
    /// with no verdict reads as `Ambiguous`, the reading of an older refmap.
    #[serde(skip_serializing_if = "crate::CaptureVerdict::is_ambiguous")]
    pub capture: crate::CaptureVerdict,
}

#[path = "ref_entry_identity_serde.rs"]
mod serde_impl;
