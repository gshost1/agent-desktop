use crate::types::notification_info::AdNotificationInfo;

/// Opaque notification list returned by `ad_list_notifications`.
pub struct AdNotificationList {
    pub(crate) items: Box<[AdNotificationInfo]>,
}
