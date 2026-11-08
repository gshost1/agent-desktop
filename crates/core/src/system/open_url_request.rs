use crate::{ProcessIdentity, WindowInfo};

/// What `open-url` asks of an adapter: the browser process to load `url` in,
/// the window whose visible tab it replaces (or that window's browser when
/// `new_tab` is set), and the deadline for the page's load.
#[derive(Debug, Clone)]
pub struct OpenUrlRequest<'a> {
    pub process: ProcessIdentity,
    pub window: Option<&'a WindowInfo>,
    pub url: &'a str,
    pub new_tab: bool,
    pub deadline: crate::Deadline,
}
