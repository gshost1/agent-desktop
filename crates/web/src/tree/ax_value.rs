use serde::Deserialize;
use serde_json::Value;

/// A CDP `AXValue`; only its `value` member is read.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct AxValue {
    #[serde(default)]
    pub(crate) value: Option<Value>,
}

impl AxValue {
    pub(crate) fn as_text(&self) -> Option<String> {
        let text = match self.value.as_ref()? {
            Value::String(text) => text.trim().to_owned(),
            Value::Bool(flag) => flag.to_string(),
            Value::Number(number) => number.to_string(),
            _ => return None,
        };
        (!text.is_empty()).then_some(text)
    }

    /// The string as the browser reported it, keeping the spaces at its ends
    /// that separate it from the text around it.
    pub(crate) fn as_untrimmed_text(&self) -> Option<String> {
        match self.value.as_ref()? {
            Value::String(text) => Some(text.clone()),
            _ => self.as_text(),
        }
    }

    pub(crate) fn is_true(&self) -> bool {
        match self.value.as_ref() {
            Some(Value::Bool(flag)) => *flag,
            Some(Value::String(text)) => text == "true",
            _ => false,
        }
    }
}
