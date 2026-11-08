/// Which channel read a snapshot's tree, reported as `data.channel`. A web
/// surface read over a browser's DevTools endpoint is `"cdp"` and names the
/// page it read; one read through the platform accessibility tree because no
/// endpoint was reachable is `"ax"` and says how to enable the web channel.
/// Snapshots of every other surface carry none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotChannel {
    pub channel: String,
    pub url: Option<String>,
    pub title: Option<String>,
    pub suggestion: Option<String>,
}

impl SnapshotChannel {
    /// Adds this channel's fields to a snapshot's `data` object.
    pub fn write_into(&self, body: &mut serde_json::Value) {
        body["channel"] = serde_json::json!(self.channel);
        for (key, value) in [
            ("url", &self.url),
            ("title", &self.title),
            ("suggestion", &self.suggestion),
        ] {
            if let Some(value) = value {
                body[key] = serde_json::json!(value);
            }
        }
    }
}

#[cfg(test)]
#[path = "snapshot_channel_tests.rs"]
mod tests;
