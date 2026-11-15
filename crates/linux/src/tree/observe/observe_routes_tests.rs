use agent_desktop_core::{AdapterError, ObservationRoot};

use super::tests::{request, window};
use super::{SettleHost, observe_with};
use crate::system::process::toolkit_id::ToolkitId;
use crate::system::window::window_id;
use crate::tree::live::test_support::{read_of, words};
use crate::tree::vocabulary::role_numbers::{FRAME, PAGE_TAB};
use crate::tree::vocabulary::state_bits::{SELECTABLE, SENSITIVE, SHOWING, VISIBLE};
use crate::tree::walk::walker::walk_levels;
use crate::tree::walk::wrapper::classify;
use crate::tree::{Expansion, Field, ObservedRead, Walk, WalkLimits};

fn gtk4() -> ToolkitId {
    ToolkitId {
        name: Some("GTK".into()),
        version: Some("4.14.5".into()),
    }
}

fn window_with_a_tab(limits: &WalkLimits) -> Result<Walk<ObservedRead>, AdapterError> {
    walk_levels(
        0u32,
        limits,
        agent_desktop_core::Deadline::after(5_000).expect("deadline"),
        |level| {
            Ok(level
                .iter()
                .map(|key| match key {
                    0 => Expansion::read(read_of(FRAME, "w", &[]), vec![1u32]),
                    _ => {
                        let mut tab = read_of(PAGE_TAB, "Tab", &[]);
                        tab.node.traits.states =
                            Field::Present(words(&[SENSITIVE, SELECTABLE, SHOWING, VISIBLE]));
                        Expansion::read(tab, Vec::new())
                    }
                })
                .collect())
        },
        classify,
    )
}

fn tab_actions(toolkit: Option<ToolkitId>) -> Vec<String> {
    let window = window(window_id::encode(":1.1", "/p").expect("encodes"));
    let tree = observe_with(
        ObservationRoot::Window(&window),
        &request(),
        SettleHost {
            walker: &mut |_, limits, _, _| window_with_a_tab(limits),
            scan: &mut |_, limits, _, _| window_with_a_tab(limits),
            place: &mut |_| Ok(toolkit.clone()),
            is_web_process: &|_| Ok(false),
            stopped: &|_| false,
            pause: &mut |_| {},
        },
    )
    .expect("the window observes")
    .into_accessibility_tree()
    .expect("one root");
    tree.children[0].presentation.available_actions.clone()
}

#[test]
fn a_snapshot_applies_the_toolkit_placement_read_to_the_tokens_it_prints() {
    assert!(!tab_actions(Some(gtk4())).contains(&"Select".to_owned()));
    assert!(tab_actions(None).contains(&"Select".to_owned()));
}
