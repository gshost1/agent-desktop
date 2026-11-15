use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListNotificationsCliArgs {
    #[arg(long, help = "Filter to notifications from this app")]
    pub app: Option<String>,
    #[arg(long, help = "Filter to notifications containing this text")]
    pub text: Option<String>,
    #[arg(long, help = "Maximum number of notifications to return")]
    pub limit: Option<usize>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DismissNotificationCliArgs {
    #[arg(value_name = "INDEX", help = "1-based notification index from list-notifications; with --key the entry must still be at INDEX",
          value_parser = clap::value_parser!(u64).range(1..), required_unless_present = "key")]
    #[serde(default)]
    pub index: Option<u64>,
    #[arg(
        long,
        value_name = "KEY",
        help = "The key list-notifications printed for the entry; without INDEX acts on that entry wherever it now sits; refuses when no entry carries it"
    )]
    #[serde(default)]
    pub key: Option<String>,
    #[arg(
        long,
        help = "Require the targeted notification (by --key or INDEX) to come from an app whose name contains this, ignoring case; INDEX counts the unfiltered list"
    )]
    pub app: Option<String>,
    #[arg(
        long,
        help = "Expected app name of the entry; without --key, INDEX needs this or --expected-title",
        value_name = "APP"
    )]
    pub expected_app: Option<String>,
    #[arg(
        long,
        help = "Expected title of the entry; without --key, INDEX needs this or --expected-app",
        value_name = "TITLE"
    )]
    pub expected_title: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DismissAllNotificationsCliArgs {
    #[arg(long, help = "Only dismiss notifications from this app")]
    pub app: Option<String>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NotificationActionCliArgs {
    #[arg(value_name = "INDEX", help = "1-based notification index from list-notifications; with --key the entry must still be at INDEX",
          value_parser = clap::value_parser!(u64).range(1..))]
    pub index: u64,
    #[arg(
        value_name = "ACTION",
        help = "Name of the action button to click (e.g., Reply, Open)"
    )]
    pub action: String,
    #[arg(
        long,
        value_name = "KEY",
        help = "The key list-notifications printed for the entry at INDEX; refuses when the entry at INDEX no longer carries it"
    )]
    #[serde(default)]
    pub key: Option<String>,
    #[arg(
        long,
        help = "Expected app name of the entry; without --key, INDEX needs this or --expected-title",
        value_name = "APP"
    )]
    pub expected_app: Option<String>,
    #[arg(
        long,
        help = "Expected title of the entry; without --key, INDEX needs this or --expected-app",
        value_name = "TITLE"
    )]
    pub expected_title: Option<String>,
}

#[cfg(test)]
#[path = "notifications_tests.rs"]
mod tests;
