use super::{read_wrapper_description, summarize_parent as parent_summary};
use crate::tree::element::ancestry::Ancestry;
use crate::tree::live::test_support::{closed_connection, read_of};
use crate::tree::vocabulary::role_numbers::{FILLER, PANEL, PUSH_BUTTON};
use crate::tree::{Field, RawNode};

fn wrapper() -> RawNode {
    let mut node = read_of(PANEL, "", &[]).node;
    node.properties.description = Field::Present("Tooltip".into());
    node.children = Field::Present(vec![(":1.1".into(), "/child".into())]);
    node
}

#[test]
fn summary_accepts_panel_and_filler() {
    for role in [PANEL, FILLER] {
        let mut node = wrapper();
        node.traits.role = Field::Present(role);
        assert_eq!(parent_summary(&node), Field::Present("Tooltip".into()));
    }
}

#[test]
fn summary_rejects_each_non_wrapper_rule() {
    for rule in 0..6 {
        let mut node = wrapper();
        match rule {
            0 => node.traits.role = Field::Present(PUSH_BUTTON),
            1 => node.properties.name = Field::Present("Named".into()),
            2 => node.children = Field::Present(vec![]),
            3 => {
                node.children =
                    Field::Present(vec![("a".into(), "/a".into()), ("b".into(), "/b".into())])
            }
            4 => node.properties.description = Field::Present("  ".into()),
            _ => node.properties.description = Field::Absent,
        }
        assert_eq!(parent_summary(&node), Field::Absent, "rule {rule}");
    }
}

#[test]
fn summary_preserves_each_failed_prerequisite() {
    for field in 0..4 {
        let mut node = wrapper();
        match field {
            0 => node.traits.role = Field::Failed("failed".into()),
            1 => node.properties.name = Field::Failed("failed".into()),
            2 => node.properties.description = Field::Failed("failed".into()),
            _ => node.children = Field::Failed("failed".into()),
        }
        assert!(parent_summary(&node).is_failed(), "field {field}");
    }
}

#[test]
fn web_document_skips_live_and_summarized_wrapper() {
    let connection = closed_connection();
    let mut node = read_of(PUSH_BUTTON, "", &["click"]).node;
    node.element.set_ancestry(Ancestry::of(None, None, true));
    for summary in [None, Some(Field::Present("Tooltip".into()))] {
        assert_eq!(
            zbus::block_on(read_wrapper_description(
                &connection,
                &node,
                summary.as_ref()
            ))
            .unwrap(),
            Field::Absent
        );
    }
}
