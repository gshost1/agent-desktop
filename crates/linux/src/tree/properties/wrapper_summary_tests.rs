use super::{WrapperParent, read_wrapper_description, summarize_parent as parent_summary};
use crate::tree::element::ancestry::Ancestry;
use crate::tree::live::test_support::{closed_connection, read_of};
use crate::tree::vocabulary::role_numbers::{FILLER, PANEL, PUSH_BUTTON};
use crate::tree::{AtspiElement, Field, RawNode};

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
fn non_wrapper_ignores_failed_text_after_readable_prerequisites() {
    for rule in 0..3 {
        let mut node = wrapper();
        node.properties.name = Field::Failed("unread name".into());
        node.properties.description = Field::Failed("unread description".into());
        match rule {
            0 => node.traits.role = Field::Present(PUSH_BUTTON),
            1 => node.children = Field::Present(vec![]),
            _ => {
                node.children =
                    Field::Present(vec![("a".into(), "/a".into()), ("b".into(), "/b".into())])
            }
        }
        assert_eq!(parent_summary(&node), Field::Absent);
    }
}

#[test]
fn web_document_skips_live_and_summarized_wrapper() {
    let connection = closed_connection();
    let mut node = read_of(PUSH_BUTTON, "", &["click"]).node;
    node.element.set_ancestry(Ancestry::of(None, None, true));
    for summary in [
        None,
        Some(super::WrapperParent::Known(Field::Present(
            "Tooltip".into(),
        ))),
    ] {
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

#[test]
fn composite_children_must_list_exactly_the_target() {
    for paths in [vec![], vec!["/other"], vec!["/child", "/other"]] {
        let (client, server) = super::tests::connection(Some(super::tests::wrapper()), false);
        let mut node = read_of(PUSH_BUTTON, "", &[]).node;
        node.element = AtspiElement::new(":1.1", "/child", 1);
        let children = paths
            .iter()
            .map(|path| (":1.1".into(), (*path).try_into().unwrap()))
            .collect();
        let summary = WrapperParent::Children(AtspiElement::new(":1.1", "/parent", 1), children);
        assert_eq!(
            zbus::block_on(read_wrapper_description(&client, &node, Some(&summary))).unwrap(),
            Field::Absent
        );
        zbus::block_on(async {
            let parent = server
                .object_server()
                .interface::<_, super::tests::Parent>("/parent")
                .await
                .unwrap();
            assert_eq!(
                parent
                    .get()
                    .await
                    .role_reads
                    .load(std::sync::atomic::Ordering::Relaxed),
                usize::from(paths.len() <= 1)
            );
        });
    }
}

#[test]
fn composite_children_preserve_role_failure_and_connection_errors() {
    let (client, _server) = super::tests::connection(None, false);
    let mut node = read_of(PUSH_BUTTON, "", &[]).node;
    node.element = AtspiElement::new(":1.1", "/child", 1);
    let summary = WrapperParent::Children(
        AtspiElement::new(":1.1", "/parent", 1),
        vec![(":1.1".into(), "/child".try_into().unwrap())],
    );
    assert!(
        zbus::block_on(read_wrapper_description(&client, &node, Some(&summary)))
            .unwrap()
            .is_failed()
    );
    let lost = closed_connection();
    let error = zbus::block_on(read_wrapper_description(&lost, &node, Some(&summary))).unwrap_err();
    assert_eq!(error.code, agent_desktop_core::ErrorCode::Internal);
}
