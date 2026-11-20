use agent_desktop_core::DisplayInfo;

/// The displays a pointer gesture is mapped onto, with the unit of Mutter's
/// monitor screen-cast streams. In Mutter's logical layout mode the listed
/// bounds are logical pixels and a stream unit is one physical pixel, so a
/// point lands at its offset times the display's scale. In the physical
/// layout mode, and on X11, the bounds are physical pixels already and the
/// stream maps one to one (L43-2).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DisplayLayout {
    pub(crate) displays: Vec<DisplayInfo>,
    pub(crate) stream_scaled: bool,
}

impl DisplayLayout {
    pub(crate) fn unscaled(displays: Vec<DisplayInfo>) -> Self {
        Self {
            displays,
            stream_scaled: false,
        }
    }

    /// Stream units per listed unit on `display`.
    pub(crate) fn stream_scale(&self, display: &DisplayInfo) -> f64 {
        if self.stream_scaled && display.scale.is_finite() && display.scale > 0.0 {
            display.scale
        } else {
            1.0
        }
    }
}
