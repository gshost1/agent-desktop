use crate::NotificationInfo;

/// What makes two listed notifications "the same notification" for a wait:
/// app, title and body. The list index is a position, and the action buttons
/// are live surface chrome that some platforms only lay out on hover or
/// expansion, so neither is part of the key. The platform adapters' own
/// same-entry checks compare the same three fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NotificationKey {
    app_name: String,
    title: String,
    body: Option<String>,
}

impl From<&NotificationInfo> for NotificationKey {
    fn from(info: &NotificationInfo) -> Self {
        Self {
            app_name: info.app_name.clone(),
            title: info.title.clone(),
            body: info.body.clone(),
        }
    }
}
