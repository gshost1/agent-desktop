use serde::Deserialize;

use super::ax_property::AxProperty;
use super::ax_text::AxText;
use super::ax_value::AxValue;

/// One node of `Accessibility.getFullAXTree`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CdpAxNode {
    pub(crate) node_id: String,
    #[serde(default)]
    pub(crate) ignored: bool,
    #[serde(default)]
    pub(crate) role: Option<AxValue>,
    #[serde(flatten)]
    pub(crate) text: AxText,
    #[serde(default)]
    pub(crate) properties: Vec<AxProperty>,
    #[serde(default)]
    pub(crate) child_ids: Vec<String>,
    #[serde(default, rename = "backendDOMNodeId")]
    pub(crate) backend_dom_node_id: Option<i64>,
}

impl CdpAxNode {
    pub(crate) fn raw_role(&self) -> String {
        self.role
            .as_ref()
            .and_then(AxValue::as_text)
            .unwrap_or_default()
    }

    pub(crate) fn property(&self, name: &str) -> Option<&AxValue> {
        self.properties
            .iter()
            .find(|property| property.name == name)
            .map(|property| &property.value)
    }
}
