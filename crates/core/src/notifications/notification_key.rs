use crate::NotificationInfo;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// What makes two listed notifications "the same notification": app, title
/// and body. The list index is a position, and the action buttons are live
/// surface chrome that some platforms only lay out on hover or expansion, so
/// neither is part of the key. Waits count entries by it and mutations find
/// their target by it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NotificationKey {
    app_name: String,
    title: String,
    body: Option<String>,
}

impl NotificationKey {
    /// A 64-bit FNV-1a digest of the three fields, stable across processes
    /// and releases so a key printed by one call is understood by the next.
    /// Each field is length-prefixed and an absent body is told apart from
    /// an empty one.
    pub(crate) fn digest(&self) -> u64 {
        let mut hash = FNV_OFFSET;
        let mut feed = |bytes: &[u8]| {
            for byte in bytes {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        };
        for field in [Some(&self.app_name), Some(&self.title), self.body.as_ref()] {
            match field {
                Some(text) => {
                    feed(&[1]);
                    feed(&(text.len() as u64).to_le_bytes());
                    feed(text.as_bytes());
                }
                None => feed(&[0]),
            }
        }
        hash
    }
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

#[cfg(test)]
#[path = "notification_key_tests.rs"]
mod tests;
