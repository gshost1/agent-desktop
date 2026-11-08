use serde::Deserialize;

/// The routing members of a DevTools frame. Every other member is skipped
/// without building it, which serde_json does iteratively, so a frame that
/// nests deeper than the 128 levels a full parse allows still yields its id.
#[derive(Debug, Deserialize)]
pub(crate) struct FrameEnvelope {
    #[serde(default)]
    pub(crate) id: Option<u64>,
}
