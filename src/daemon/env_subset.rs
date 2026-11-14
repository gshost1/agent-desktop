//! The environment variables that decide what a command sees: the display and bus it talks to,
//! the state root, every `AGENT_DESKTOP_*` setting, locale, paths and toolkit switches. A daemon
//! serves only clients whose values match its own, so a command never runs against another
//! display, bus or state root than the one its caller named.

use super::OPT_IN_ENV;

const EXACT_NAMES: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "AT_SPI_BUS_ADDRESS",
    "NO_AT_BRIDGE",
    "HOME",
    "USER",
    "LOGNAME",
    "PATH",
    "SHELL",
    "TMPDIR",
    "TMP",
    "TEMP",
    "LANG",
    "LANGUAGE",
    "TZ",
    "GSETTINGS_BACKEND",
    "TERM_PROGRAM",
    "__CFBundleIdentifier",
];

const PREFIXES: &[&str] = &[
    "AGENT_DESKTOP_",
    "XDG_",
    "DBUS_",
    "LC_",
    "GTK_",
    "GDK_",
    "QT_",
    "RUST_",
];

/// The relevant variables of one process, sorted by name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct EnvSubset(Vec<(String, String)>);

impl EnvSubset {
    /// This process's relevant variables; `None` when one of them is not valid UTF-8.
    pub(crate) fn current() -> Option<Self> {
        let mut pairs = Vec::new();
        for (name, value) in std::env::vars_os() {
            let Some(name) = name.to_str() else {
                continue;
            };
            if is_relevant(name) {
                pairs.push((name.to_owned(), value.into_string().ok()?));
            }
        }
        Some(Self::from_pairs(pairs))
    }

    pub(crate) fn from_pairs(mut pairs: Vec<(String, String)>) -> Self {
        pairs.retain(|(name, _)| is_relevant(name));
        pairs.sort();
        Self(pairs)
    }

    pub(crate) fn pairs(&self) -> &[(String, String)] {
        &self.0
    }
}

/// Whether `name` can change what a command does. The daemon's own control variables are left
/// out except the idle timeout, which shapes the daemon itself and so keys it.
pub(crate) fn is_relevant(name: &str) -> bool {
    if name == OPT_IN_ENV || name == super::SERVE_ENV {
        return false;
    }
    EXACT_NAMES.contains(&name) || PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

#[cfg(test)]
#[path = "env_subset_tests.rs"]
mod tests;
