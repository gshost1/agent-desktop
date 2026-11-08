use serde::Deserialize;

use super::ax_value::AxValue;

/// The computed text members of a CDP `AXNode`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct AxText {
    #[serde(default)]
    pub(crate) name: Option<AxValue>,
    #[serde(default)]
    pub(crate) description: Option<AxValue>,
    #[serde(default)]
    pub(crate) value: Option<AxValue>,
}
