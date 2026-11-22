use super::summarize;
use super::tests::{buttons, filler, node, only_region, parent, window};
use crate::refs::ref_alloc::transform_tree;

#[test]
fn own_vertical_evidence_controls_partial() {
    for evidence in [None, Some(false), Some(true)] {
        let mut list = parent(node("list", ""), buttons(3, "r"));
        list.presentation.available_actions = vec!["Scroll".into()];
        list.presentation.scrolls_vertically = evidence;
        assert_eq!(
            only_region(&window(vec![filler(), list])).partial,
            evidence == Some(true)
        );
    }
}

#[test]
fn only_single_child_wrappers_inherit_scroll_evidence() {
    for depth in 0..=4 {
        for multiple in [false, true] {
            let mut child = parent(node("list", ""), buttons(3, "r"));
            for _ in 0..depth {
                let mut children = vec![child];
                if multiple {
                    children.push(node("statictext", "sibling"));
                }
                child = parent(node("group", ""), children);
            }
            let mut area = parent(node("scrollarea", ""), vec![child]);
            area.presentation.scrolls_vertically = Some(true);
            let contents = summarize(&window(vec![filler(), area])).unwrap();
            let list = contents.regions.iter().find(|r| r.role == "list").unwrap();
            assert_eq!(
                list.partial,
                depth <= 3 && (depth == 0 || !multiple),
                "depth={depth}, multiple={multiple}"
            );
        }
    }
}

#[test]
fn compact_hoists_scroll_evidence_onto_the_survivor() {
    for outer in [None, Some(false), Some(true)] {
        for own in [None, Some(false), Some(true)] {
            let mut list = parent(node("list", ""), buttons(3, "r"));
            list.presentation.scrolls_vertically = own;
            let mut area = parent(node("group", ""), vec![list]);
            area.presentation.scrolls_vertically = outer;
            let tree = transform_tree(window(vec![filler(), area]), false, false, true);
            assert_eq!(tree.children[1].role, "list");
            assert_eq!(only_region(&tree).partial, own.or(outer) == Some(true));
        }
    }
}

#[test]
fn only_a_scroll_areas_single_list_descendant_inherits_its_evidence() {
    for count in [0, 1, 3] {
        for nested in [false, true] {
            let mut lists: Vec<_> = (0..count)
                .map(|index| parent(node("list", &format!("list-{index}")), buttons(3, "r")))
                .collect();
            if nested && count == 3 {
                let last = lists.pop().unwrap();
                lists.push(parent(node("group", ""), vec![last]));
            }
            let mut area = parent(node("scrollarea", ""), lists);
            area.presentation.scrolls_vertically = Some(true);
            let contents = summarize(&window(vec![filler(), area])).unwrap();
            let regions: Vec<_> = contents
                .regions
                .iter()
                .filter(|region| region.role == "list")
                .collect();
            assert_eq!(regions.len(), count);
            assert!(
                regions.iter().all(|region| region.partial == (count == 1)),
                "count={count}, nested={nested}"
            );
        }
    }
}

#[test]
fn regions_rank_largest_first_and_drop_trivial_lists() {
    let mut children = vec![filler(), parent(node("toolbar", ""), vec![])];
    children.extend((1..=7).map(|n| parent(node("list", &format!("L{n}")), buttons(n, "r"))));
    children.push(parent(node("list", "Large"), buttons(20, "large")));
    let contents = summarize(&window(children)).unwrap();
    assert_eq!(contents.regions[0].name.as_deref(), Some("Large"));
    assert_eq!(
        contents.regions.iter().map(|r| r.items).collect::<Vec<_>>(),
        vec![Some(20), Some(7), Some(6), Some(5), Some(4), Some(3)]
    );
    assert_eq!(contents.more_regions, 1);
}
