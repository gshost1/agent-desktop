use super::*;
use crate::tree::vocabulary::state_bits::SHOWING;

#[test]
fn mixed_child_visibility_reaches_list_evidence_before_output_projection() {
    for role in [31, 55, 65, 66, 98, 121] {
        let limits = WalkLimits::default();
        let edges = HashMap::from([(0, vec![1, 2])]);
        let walked = walk(
            &edges,
            |key| {
                let mut read = read_of(if key == 0 { role } else { 32 }, "", &[]);
                read.node.traits.states =
                    Field::Present(vec![if key == 2 { 0 } else { 1 << SHOWING }, 0]);
                read
            },
            &limits,
        );
        assert_eq!(walked.root.children.len(), 2);
        let tree = tree_of(walked, &limits);
        assert_eq!(tree.presentation.scrolls_vertically, Some(true), "{role}");
    }
}
