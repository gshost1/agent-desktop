use agent_desktop_core::Rect;

const TOLERANCE: f64 = 2.0;
const MIN_SCALE: f64 = 1.0;
const MAX_SCALE: f64 = 5.0;

/// How a browser window's bounds, which the browser reports in DIPs, fit a
/// platform window's bounds, which the platform reports in its own unit.
/// Variants order from the closest fit to the loosest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BoundsFit {
    /// Every edge agrees in the same unit, as when the platform reports
    /// points or DIPs.
    Exact,
    /// Origin and size agree once divided by one scale factor, as when the
    /// platform reports device pixels of a display scaled by that factor.
    Scaled,
    /// Only the size agrees at one scale factor. The origin does not, as on
    /// a display whose scale differs from the primary display's, or on a
    /// platform that reports no window origin.
    Sized,
}

impl BoundsFit {
    /// The fit of `browser` (DIPs) to `platform`, or `None` when no scale
    /// factor between 1 and 5 maps one size onto the other.
    pub(crate) fn of(browser: &Rect, platform: &Rect) -> Option<Self> {
        if (0..4).all(|edge| (edges(browser)[edge] - edges(platform)[edge]).abs() <= TOLERANCE) {
            return Some(Self::Exact);
        }
        let scale = size_scale(browser, platform)?;
        let slack = 2.0 * TOLERANCE / (browser.width + browser.height);
        let origin_agrees =
            |dip: f64, device: f64| (device / scale - dip).abs() <= TOLERANCE + dip.abs() * slack;
        if origin_agrees(browser.x, platform.x) && origin_agrees(browser.y, platform.y) {
            Some(Self::Scaled)
        } else {
            Some(Self::Sized)
        }
    }
}

fn edges(rect: &Rect) -> [f64; 4] {
    [rect.x, rect.y, rect.width, rect.height]
}

fn size_scale(browser: &Rect, platform: &Rect) -> Option<f64> {
    if browser.width < 1.0 || browser.height < 1.0 {
        return None;
    }
    let scale = (platform.width + platform.height) / (browser.width + browser.height);
    let in_range = (MIN_SCALE..=MAX_SCALE).contains(&scale);
    let width_agrees = (platform.width / scale - browser.width).abs() <= TOLERANCE;
    let height_agrees = (platform.height / scale - browser.height).abs() <= TOLERANCE;
    (in_range && width_agrees && height_agrees).then_some(scale)
}

#[cfg(test)]
#[path = "bounds_fit_tests.rs"]
mod tests;
