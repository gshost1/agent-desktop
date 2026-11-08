use crate::NotificationInfo;

#[derive(Debug, Clone, Default)]
pub struct NotificationFilter {
    pub app: Option<String>,
    pub text: Option<String>,
    pub limit: Option<usize>,
}

impl NotificationFilter {
    /// Whether `info` passes the app and text predicates: the app name, or
    /// the title, body and app name together, contain the filter text,
    /// ignoring case. The limit is applied by the caller after this.
    pub fn admits(&self, info: &NotificationInfo) -> bool {
        let text_ok = self.text.as_deref().is_none_or(|text| {
            let body = info.body.as_deref().unwrap_or("");
            format!("{} {body} {}", info.title, info.app_name)
                .to_lowercase()
                .contains(&text.to_lowercase())
        });
        self.app.as_deref().is_none_or(|app| app_matches(info, app)) && text_ok
    }
}

/// Whether the app name contains `app`, ignoring case: the meaning of every
/// `--app` notification filter.
pub(crate) fn app_matches(info: &NotificationInfo, app: &str) -> bool {
    info.app_name.to_lowercase().contains(&app.to_lowercase())
}

#[cfg(test)]
#[path = "notification_filter_tests.rs"]
mod tests;
