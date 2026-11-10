use serde::Serialize;

/// One `find` match as it is printed. A context match has no `ref_id`, and
/// absent values and empty `states` and `path` are left out, as everywhere
/// else in the JSON contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocatorMatchData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    pub role: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    pub interactive: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
}
