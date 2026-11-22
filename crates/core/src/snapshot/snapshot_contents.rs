//! A short index printed before a snapshot's tree: how many of each kind of
//! control it holds and where each list and bar starts, so a read cut short
//! still shows what exists and where.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{content_region::ContentRegion, contents_walk::Walk};
use crate::AccessibilityNode;

const MAX_ROLES: usize = 4;
const MAX_REGIONS: usize = 6;
const MIN_NODES: usize = 80;
const MIN_LIST_ITEMS: usize = 3;
const MIN_SCROLLED_ITEMS: usize = 5;
const MAX_NAME_CHARS: usize = 32;
pub(super) const LIST_ROLES: &[&str] = &["list", "listbox", "table", "outline", "grid", "browser"];
const BAR_ROLES: &[&str] = &["toolbar", "tablist"];
const NEVER_ITEM_ROLES: &[&str] = &["column", "scrollbar"];
const NON_ROW_ROLES: &[&str] = &["column", "scrollbar", "statictext"];
pub(super) const STRUCTURE_ROLES: &[&str] =
    &["column", "handle", "scrollarea", "scrollbar", "window"];
const SAME_ROLE_SHARE: f64 = 0.8;
pub(crate) const MAX_WRAPPER_DEPTH: usize = 3;

/// What a snapshot holds: the most common roles that carry refs, by count,
/// and the lists and bars, largest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotContents {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roles: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<ContentRegion>,
    /// Regions past the first few, left out to keep the index short.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub more_regions: usize,
}

impl SnapshotContents {
    /// The `--lines` form: the role counts on one line, then one line per
    /// region, each led by the line its node prints on.
    pub fn lines(&self) -> Vec<String> {
        let counts: Vec<String> = self
            .roles
            .iter()
            .map(|(role, count)| format!("{role} {count}"))
            .collect();
        let mut lines = Vec::new();
        if !counts.is_empty() {
            lines.push(format!("roles: {}", counts.join(", ")));
        }
        lines.extend(self.regions.iter().map(region_line));
        if self.more_regions > 0 {
            lines.push(format!("+{} more regions", self.more_regions));
        }
        lines
    }
}

fn region_line(region: &ContentRegion) -> String {
    let mut line = format!("line {}:", region.line);
    if let Some(ref_id) = &region.ref_id {
        line.push(' ');
        line.push_str(ref_id);
    }
    line.push(' ');
    line.push_str(&region.role);
    if let Some(name) = &region.name {
        line.push(' ');
        line.push_str(&super::ref_lines::quote_text(name));
    }
    if let Some(items) = region.items {
        line.push_str(&format!(" items={items}"));
    }
    if region.partial {
        line.push_str(" partial");
    }
    line
}

fn is_zero(count: &usize) -> bool {
    *count == 0
}

/// The contents of `tree`, or `None` for a tree small enough to read whole
/// or one with nothing to index.
pub fn summarize(tree: &AccessibilityNode) -> Option<SnapshotContents> {
    let mut walk = Walk::default();
    walk.visit(tree, None);
    if walk.line < MIN_NODES || (walk.roles.is_empty() && walk.regions.is_empty()) {
        return None;
    }
    let mut ranked: Vec<(String, usize)> = walk.roles.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(MAX_ROLES);
    walk.regions.retain(|region| {
        region.items.is_none_or(|items| items >= MIN_LIST_ITEMS) || region.partial
    });
    walk.regions.sort_by(|a, b| {
        b.items
            .unwrap_or(0)
            .cmp(&a.items.unwrap_or(0))
            .then_with(|| a.line.cmp(&b.line))
    });
    let more_regions = walk.regions.len().saturating_sub(MAX_REGIONS);
    walk.regions.truncate(MAX_REGIONS);
    Some(SnapshotContents {
        roles: ranked.into_iter().collect(),
        regions: walk.regions,
        more_regions,
    })
}

pub(super) fn region_of(
    node: &AccessibilityNode,
    line: usize,
    scroll_area: Option<&AccessibilityNode>,
) -> Option<ContentRegion> {
    let role = node.role.as_str();
    let (items, partial) = if BAR_ROLES.contains(&role) {
        (None, false)
    } else if LIST_ROLES.contains(&role) {
        let partial = was_cut(node)
            || node.presentation.scrolls_vertically == Some(true)
            || scroll_area.is_some_and(|area| {
                area.presentation.scrolls_vertically == Some(true) || was_cut(area)
            });
        (Some(list_items(node)), partial)
    } else if role == "scrollarea" && !wraps_a_list(node) {
        let content = unwrapped(&node.children);
        let count = row_count(content)?;
        let partial = node.presentation.scrolls_vertically == Some(true)
            || was_cut(node)
            || content.iter().any(was_cut);
        (Some(count), partial)
    } else {
        return None;
    };
    Some(ContentRegion {
        line,
        role: node.role.clone(),
        name: node
            .identity
            .name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .map(short_name),
        ref_id: node.ref_id.clone(),
        items,
        partial,
    })
}

fn wraps_a_list(area: &AccessibilityNode) -> bool {
    let mut level = &area.children[..];
    for _ in 0..=MAX_WRAPPER_DEPTH {
        if level
            .iter()
            .any(|child| LIST_ROLES.contains(&child.role.as_str()))
        {
            return true;
        }
        match content_of(level).as_slice() {
            [only] => level = &only.children,
            _ => return false,
        }
    }
    false
}

pub(super) fn content_of(children: &[AccessibilityNode]) -> Vec<&AccessibilityNode> {
    children
        .iter()
        .filter(|child| child.role != "scrollbar")
        .collect()
}

fn unwrapped(children: &[AccessibilityNode]) -> &[AccessibilityNode] {
    let mut items = children;
    for _ in 0..MAX_WRAPPER_DEPTH {
        match content_of(items).as_slice() {
            [only] if !only.children.is_empty() => items = &only.children,
            _ => break,
        }
    }
    items
}

fn list_items(list: &AccessibilityNode) -> usize {
    match list.children_count {
        Some(count) if list.children.is_empty() => count as usize,
        _ => list
            .children
            .iter()
            .filter(|child| !NEVER_ITEM_ROLES.contains(&child.role.as_str()))
            .count(),
    }
}

fn row_count(items: &[AccessibilityNode]) -> Option<usize> {
    let mut by_role: BTreeMap<&str, usize> = BTreeMap::new();
    for item in items {
        if !NON_ROW_ROLES.contains(&item.role.as_str()) {
            *by_role.entry(item.role.as_str()).or_default() += 1;
        }
    }
    let count: usize = by_role.values().sum();
    let commonest = by_role.values().copied().max().unwrap_or(0);
    (count >= MIN_SCROLLED_ITEMS && commonest as f64 >= SAME_ROLE_SHARE * count as f64)
        .then_some(count)
}

fn was_cut(node: &AccessibilityNode) -> bool {
    node.subtree_truncated
        || node
            .children_count
            .is_some_and(|count| count as usize > node.children.len())
}

fn short_name(name: &str) -> String {
    if name.chars().count() <= MAX_NAME_CHARS {
        return name.to_string();
    }
    let mut short: String = name.chars().take(MAX_NAME_CHARS - 1).collect();
    short.push('…');
    short
}

#[cfg(test)]
#[path = "snapshot_contents_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "contents_evidence_tests.rs"]
mod evidence_tests;
