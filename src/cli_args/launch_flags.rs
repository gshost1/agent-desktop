use clap::Args;
use serde::Deserialize;

/// How `launch` treats an app that is already running: attach to it or start
/// a fresh instance, bring it forward, or send it a reopen.
#[derive(Args, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LaunchAttachArgs {
    #[arg(
        long = "no-attach",
        help = "Start a fresh instance instead of attaching to a running one (Windows fails if the app is already running)"
    )]
    #[serde(default)]
    pub no_attach: bool,
    #[arg(
        long,
        help = "Bring the app forward so it presents a window, and wait for one"
    )]
    #[serde(default)]
    pub activate: bool,
    #[arg(
        long,
        help = "Send a running app a reopen so one with no window opens its default window (macOS; fails on Windows and Linux)"
    )]
    #[serde(default)]
    pub reopen: bool,
}

/// The DevTools flags of `launch`: the endpoint port and the folder the
/// browser saves downloads to.
#[derive(Args, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LaunchCdpArgs {
    #[arg(
        long,
        value_name = "PORT",
        num_args = 0..=1,
        default_missing_value = "0",
        help = "Expose the app's DevTools protocol endpoint on this port (0 or omitted picks a free port)"
    )]
    #[serde(default)]
    pub cdp: Option<u16>,
    #[arg(
        long = "download-dir",
        value_name = "DIR",
        requires = "cdp",
        help = "With --cdp: save the browser's downloads in this folder, created if missing; wait --download returns each saved file"
    )]
    #[serde(default)]
    pub download_dir: Option<std::path::PathBuf>,
}
