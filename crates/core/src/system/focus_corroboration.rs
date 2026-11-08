//! The verdict of core's one read after an adapter confirmed a window focused.

use serde_json::json;

use crate::{AdapterError, AppError, DeliverySemantics, ErrorCode, WindowInfo};

/// What the corroborating read saw once the adapter had confirmed focus.
#[derive(Debug)]
pub(crate) enum FocusCorroboration {
    Held(WindowInfo),
    Disagreed(Option<String>),
    Unread(AdapterError),
}

impl FocusCorroboration {
    pub(crate) fn judge(target_id: &str, read: Result<Option<WindowInfo>, AdapterError>) -> Self {
        match read {
            Ok(Some(window)) if window.id == target_id => Self::Held(window),
            Ok(observed) => Self::Disagreed(observed.map(|window| window.id)),
            Err(error) => Self::Unread(error),
        }
    }

    pub(crate) fn into_result(self) -> Result<WindowInfo, AppError> {
        match self {
            Self::Held(window) => Ok(window),
            Self::Disagreed(observed) => Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Window focus did not hold on the requested window after the platform confirmed it",
            )
            .with_details(json!({ "observed_window_id": observed }))
            .with_suggestion("Run 'list-windows' to refresh window IDs, then retry.")
            .with_disposition(DeliverySemantics::delivered_unverified())
            .into()),
            Self::Unread(error) => Err(error
                .with_disposition(DeliverySemantics::delivered_unverified())
                .into()),
        }
    }
}

#[cfg(test)]
#[path = "focus_corroboration_tests.rs"]
mod tests;
