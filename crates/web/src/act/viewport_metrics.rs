use agent_desktop_core::{AdapterError, Deadline, Point};
use serde_json::{Value, json};

use super::hit_point::HitPoint;
use crate::{CdpConnection, PageSession};

/// The visible part of a page in CSS pixels, read from
/// `Page.getLayoutMetrics`: where it sits in the document, its size, and its
/// pinch zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewportMetrics {
    pub page_x: f64,
    pub page_y: f64,
    pub width: f64,
    pub height: f64,
    pub zoom: f64,
}

impl ViewportMetrics {
    pub(crate) fn read(
        connection: &mut CdpConnection,
        session: &PageSession,
        deadline: Deadline,
    ) -> Result<Self, AdapterError> {
        let reply = connection.call(
            "Page.getLayoutMetrics",
            json!({}),
            Some(&session.session_id),
            deadline,
        )?;
        let visual = reply
            .get("cssVisualViewport")
            .ok_or_else(|| AdapterError::internal("Page.getLayoutMetrics returned no viewport"))?;
        let number = |key: &str| visual.get(key).and_then(Value::as_f64);
        Ok(Self {
            page_x: number("pageX").unwrap_or(0.0),
            page_y: number("pageY").unwrap_or(0.0),
            width: number("clientWidth").unwrap_or(f64::INFINITY),
            height: number("clientHeight").unwrap_or(f64::INFINITY),
            zoom: number("zoom").filter(|zoom| *zoom > 0.0).unwrap_or(1.0),
        })
    }

    /// The viewport point paired with where it falls in the document.
    pub(crate) fn hit_point(&self, viewport: Point) -> HitPoint {
        let document = Point {
            x: self.page_x + viewport.x / self.zoom,
            y: self.page_y + viewport.y / self.zoom,
        };
        HitPoint { viewport, document }
    }

    /// The centre of the part of `quad` the viewport shows, or `None` when
    /// none of it is visible or the quad is malformed.
    pub(crate) fn visible_center(&self, quad: &Value) -> Option<Point> {
        let numbers: Vec<f64> = quad.as_array()?.iter().filter_map(Value::as_f64).collect();
        if numbers.len() != 8 {
            return None;
        }
        let xs = [numbers[0], numbers[2], numbers[4], numbers[6]];
        let ys = [numbers[1], numbers[3], numbers[5], numbers[7]];
        let left = xs.iter().copied().fold(f64::INFINITY, f64::min).max(0.0);
        let right = xs
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            .min(self.width);
        let top = ys.iter().copied().fold(f64::INFINITY, f64::min).max(0.0);
        let bottom = ys
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            .min(self.height);
        if right <= left || bottom <= top {
            return None;
        }
        Some(Point {
            x: (left + right) / 2.0,
            y: (top + bottom) / 2.0,
        })
    }
}

#[cfg(test)]
#[path = "viewport_metrics_tests.rs"]
mod tests;
