use super::TraversalArena;
use crate::tree::{AXElement, element_for_pid};
use core_foundation::base::{CFGetRetainCount, CFTypeRef};

fn retain_count(element: &AXElement) -> isize {
    unsafe { CFGetRetainCount(element.0 as CFTypeRef) }
}

#[test]
fn an_element_on_the_ancestor_path_stays_retained_until_the_walk_leaves_it() {
    let element = element_for_pid(std::process::id() as i32);
    let before = retain_count(&element);
    let mut arena = TraversalArena::new();

    assert!(arena.enter(&element));
    assert!(
        !arena.enter(&element),
        "an element already on the path is a cycle"
    );
    assert_eq!(retain_count(&element), before + 1);

    arena.leave(&element);
    assert_eq!(retain_count(&element), before);
    assert!(arena.enter(&element), "a left element may be entered again");
    arena.leave(&element);
}

#[test]
fn a_resumed_path_keeps_every_ancestor_it_compares_against_alive() {
    let element = element_for_pid(std::process::id() as i32);
    let before = retain_count(&element);
    let mut arena = TraversalArena::new();
    assert!(arena.enter(&element));
    let saved = arena.ancestor_path();
    arena.leave(&element);

    assert_eq!(
        retain_count(&element),
        before + 1,
        "a path saved for a later read must hold its ancestors, or their pointers can be reused"
    );
    let previous = arena.replace_ancestor_path(saved);
    assert!(
        !arena.enter(&element),
        "the resumed path guards its ancestors"
    );
    let resumed = arena.replace_ancestor_path(previous);
    drop(resumed);
    assert_eq!(retain_count(&element), before);
    assert!(arena.enter(&element));
    arena.leave(&element);
}
