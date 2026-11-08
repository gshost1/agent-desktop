/// The identity a ref is re-identified by. It is written and read by hand
/// (`ref_entry_identity_serde.rs`) so the capture verdict always travels as
/// `capture`, and a refmap from before the verdict still reads through its
/// `unique_at_capture` flag.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RefEntryIdentity {
    pub retained_object: Option<String>,
    pub role: String,
    pub name: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub native_id: Option<crate::ElementIdentifier>,
    /// What the capture this entry was published from proved about its
    /// identity. In memory an entry no capture judged is `Unknown`, the
    /// `Default`. On disk the verdict is always written; an entry without it
    /// comes from a refmap that predates the verdict and reads as its
    /// `unique_at_capture` flag says, `Ambiguous` when the flag is false or
    /// missing.
    pub capture: crate::CaptureVerdict,
}

#[path = "ref_entry_identity_serde.rs"]
mod serde_impl;
