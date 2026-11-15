/// A pointer position in the coordinates of one linked screen-cast stream:
/// `stream` is the index of the stream in link order, and `x`, `y` are the
/// stream's own units, which Mutter divides by the monitor's scale.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StreamPoint {
    pub(crate) stream: usize,
    pub(crate) x: f64,
    pub(crate) y: f64,
}
