use agent_desktop_core::Point;

/// One place a press could land, in the two spaces CDP uses for it:
/// `viewport` is the main frame viewport point `Input.dispatchMouseEvent`
/// takes, `document` is the same point in the scrolled document, the space
/// `DOM.getNodeForLocation` hit-tests in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HitPoint {
    pub viewport: Point,
    pub document: Point,
}
