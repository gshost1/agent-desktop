use serde::Deserialize;

use super::ax_value::AxValue;

/// One entry of a CDP `AXNode.properties` list.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AxProperty {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) value: AxValue,
}
