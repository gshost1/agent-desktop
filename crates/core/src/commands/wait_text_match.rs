use crate::{AccessibilityNode, tree::search_text};

pub(crate) struct TextMatch {
    pub ref_id: Option<String>,
    pub role: String,
}

pub(crate) fn find(
    node: &AccessibilityNode,
    text_lower: &str,
    expected_count: Option<usize>,
) -> Vec<TextMatch> {
    let mut matches = Vec::new();
    collect(node, text_lower, match_limit(expected_count), &mut matches);
    matches
}

/// Says what a refmap cut left of a text match. The wait matched against the
/// tree it observed; `stored` holds the matches in the tree the saved snapshot
/// kept. A cut keeps a prefix in document order, so the first observed match
/// is in the saved tree exactly when any match is. `match_cut` says the match
/// lies past the cut, so `ref` is absent because the snapshot does not hold
/// it. With a count, `count` stays what the complete read found and
/// `stored_count` says how many of those the saved snapshot holds.
pub(crate) fn describe_refmap_cut(
    body: &mut serde_json::Value,
    refmap_capped: bool,
    stored: &[TextMatch],
    counted: bool,
) {
    if !refmap_capped {
        return;
    }
    body["complete"] = serde_json::json!(false);
    body["refmap_capped"] = serde_json::json!(true);
    if stored.is_empty() {
        body["match_cut"] = serde_json::json!(true);
    }
    if counted {
        body["stored_count"] = serde_json::json!(stored.len());
    }
}

fn match_limit(expected_count: Option<usize>) -> usize {
    expected_count
        .map(|expected| expected.saturating_add(1))
        .unwrap_or(1)
}

fn collect(node: &AccessibilityNode, text_lower: &str, limit: usize, matches: &mut Vec<TextMatch>) {
    if matches.len() >= limit {
        return;
    }
    if search_text::node_contains(node, text_lower) {
        matches.push(TextMatch {
            ref_id: node.ref_id.clone(),
            role: node.role.clone(),
        });
    }

    for child in &node.children {
        collect(child, text_lower, limit, matches);
        if matches.len() >= limit {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, children: Vec<AccessibilityNode>) -> AccessibilityNode {
        AccessibilityNode {
            ref_id: None,
            role: "group".into(),
            identity: crate::NodeIdentity {
                retained_object: None,
                name: Some(name.into()),
                ..Default::default()
            },
            presentation: Default::default(),
            children_count: None,
            subtree_truncated: false,
            children,
        }
    }

    #[test]
    fn default_wait_text_stops_after_first_match() {
        let tree = node(
            "root",
            vec![
                node("ready one", vec![]),
                node("ready two", vec![]),
                node("ready three", vec![]),
            ],
        );

        let matches = find(&tree, "ready", None);

        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn count_wait_text_collects_one_past_expected() {
        let tree = node(
            "root",
            vec![
                node("ready one", vec![]),
                node("ready two", vec![]),
                node("ready three", vec![]),
            ],
        );

        let matches = find(&tree, "ready", Some(2));

        assert_eq!(matches.len(), 3);
    }
}
