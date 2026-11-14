use crate::NotificationInfo;

#[derive(Debug, Clone, Default)]
pub struct NotificationIdentity {
    pub expected_app: Option<String>,
    pub expected_title: Option<String>,
}

impl NotificationIdentity {
    pub fn is_empty(&self) -> bool {
        self.expected_app.as_deref().is_none_or(str::is_empty)
            && self.expected_title.as_deref().is_none_or(str::is_empty)
    }

    /// Whether `info` carries every non-empty expected field; an empty
    /// field expects nothing, as in [`Self::is_empty`].
    pub fn matches(&self, info: &NotificationInfo) -> bool {
        let agrees = |expected: &Option<String>, actual: &str| {
            expected
                .as_deref()
                .is_none_or(|expected| expected.is_empty() || expected == actual)
        };
        agrees(&self.expected_app, &info.app_name) && agrees(&self.expected_title, &info.title)
    }
}

#[cfg(test)]
#[path = "notification_identity_tests.rs"]
mod tests;
