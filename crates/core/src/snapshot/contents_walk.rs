use std::collections::BTreeMap;

use super::{
    content_region::ContentRegion,
    snapshot_contents::{LIST_ROLES, MAX_WRAPPER_DEPTH, STRUCTURE_ROLES, content_of, region_of},
};
use crate::AccessibilityNode;

#[derive(Default)]
pub(crate) struct Walk {
    pub(crate) line: usize,
    pub(crate) roles: BTreeMap<String, usize>,
    pub(crate) regions: Vec<ContentRegion>,
}

impl Walk {
    pub(crate) fn visit(
        &mut self,
        node: &AccessibilityNode,
        scroll_area: Option<(&AccessibilityNode, usize)>,
    ) {
        let line = self.line;
        self.line += 1;
        if node.ref_id.is_some() && !STRUCTURE_ROLES.contains(&node.role.as_str()) {
            *self.roles.entry(node.role.clone()).or_default() += 1;
        }
        if let Some(region) = region_of(node, line, scroll_area.map(|(area, _)| area)) {
            self.regions.push(region);
        }
        let role = node.role.as_str();
        let area = if role == "scrollarea" {
            has_one_list(node).then_some((node, 0))
        } else if !LIST_ROLES.contains(&role) && content_of(&node.children).len() == 1 {
            scroll_area
                .and_then(|(area, depth)| (depth < MAX_WRAPPER_DEPTH).then_some((area, depth + 1)))
        } else {
            None
        };
        for child in &node.children {
            self.visit(child, area);
        }
    }
}

fn has_one_list(area: &AccessibilityNode) -> bool {
    let mut pending: Vec<_> = area.children.iter().collect();
    let mut lists = 0;
    while let Some(node) = pending.pop() {
        if LIST_ROLES.contains(&node.role.as_str()) {
            lists += 1;
            if lists > 1 {
                return false;
            }
        }
        pending.extend(&node.children);
    }
    lists == 1
}
