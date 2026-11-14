use crate::convert::string::optional_adapter_string;
use crate::types::AdNotificationFilter;
use agent_desktop_core::AdapterError;
use agent_desktop_core::NotificationFilter;

/// Converts a C `AdNotificationFilter` into the core filter type.
///
/// - `null` pointer → `Ok(NotificationFilter::default())` (no filter).
/// - `has_limit == false` → the numeric limit is cleared regardless of
///   `limit` contents.
/// - Non-null `app` / `text` with invalid UTF-8 → `Err` rather than
///   silently dropping the filter (which would widen operations like
///   `ad_dismiss_all_notifications` to every app on the system).
///
/// # Safety
/// `filter` must be null or point to a valid `AdNotificationFilter`.
/// The embedded C strings must outlive this call.
pub(crate) unsafe fn filter_from_c(
    filter: *const AdNotificationFilter,
) -> Result<NotificationFilter, AdapterError> {
    if filter.is_null() {
        return Ok(NotificationFilter::default());
    }
    let f: &AdNotificationFilter = unsafe { &*filter };
    let app = optional_adapter_string(f.app, "filter.app")?;
    let text = optional_adapter_string(f.text, "filter.text")?;
    Ok(NotificationFilter {
        app,
        text,
        limit: if f.has_limit {
            Some(f.limit as usize)
        } else {
            None
        },
    })
}
