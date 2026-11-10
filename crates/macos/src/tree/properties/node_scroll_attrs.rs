/// Scroll evidence read in the same batch as a node's other attributes.
/// `content_size` is `AXContentSize` as width and height, present only on
/// elements that publish it, such as SwiftUI scroll areas.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct NodeScrollAttrs {
    pub(crate) has_scrollbars: bool,
    pub(crate) content_size: Option<(f64, f64)>,
}
