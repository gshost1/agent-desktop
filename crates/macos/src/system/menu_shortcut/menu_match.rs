use crate::tree::AXElement;

/// A menu item whose published shortcut equals the requested chord, with the
/// titles the press report names.
pub(crate) struct MenuMatch {
    pub(crate) element: AXElement,
    pub(crate) menu: Option<String>,
    pub(crate) item: Option<String>,
    pub(crate) enabled: bool,
}
