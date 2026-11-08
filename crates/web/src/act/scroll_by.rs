use agent_desktop_core::{AdapterError, Deadline};
use serde_json::json;

use super::element_call::ElementCall;
use super::remote_element::call_on_element;
use crate::{CdpConnection, ElementToken, PageSession};

const SCROLL_BY: &str = r"function(dx, dy) {
  const doc = this.ownerDocument;
  const scrolls = (node) => {
    if (!node || node.nodeType !== 1) return false;
    const style = doc.defaultView.getComputedStyle(node);
    const overflow = style.overflowY + ' ' + style.overflowX;
    return /(auto|scroll|overlay)/.test(overflow)
      && (node.scrollHeight > node.clientHeight || node.scrollWidth > node.clientWidth);
  };
  let target = this;
  while (target && !scrolls(target)) target = target.parentElement;
  if (!target) target = doc.scrollingElement || doc.documentElement;
  const before = [target.scrollLeft, target.scrollTop];
  target.scrollBy(dx, dy);
  return target.scrollLeft !== before[0] || target.scrollTop !== before[1];
}";

/// Scrolls the node's nearest scrollable container, or the document, by
/// `(dx, dy)` CSS pixels. Returns whether the scroll position changed.
pub fn scroll_by(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    delta: (f64, f64),
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let moved = call_on_element(
        connection,
        session,
        token,
        ElementCall {
            function: SCROLL_BY,
            arguments: vec![json!(delta.0), json!(delta.1)],
        },
        deadline,
    )?;
    Ok(moved.as_bool().unwrap_or(false))
}

#[cfg(test)]
#[path = "scroll_by_tests.rs"]
mod tests;
