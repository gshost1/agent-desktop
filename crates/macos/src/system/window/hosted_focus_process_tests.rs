use super::*;

#[test]
fn a_process_is_never_its_own_responsible_child() {
    let own = std::process::id() as i32;

    assert!(!responsible_children(own).contains(&own));
}

#[test]
fn every_responsible_child_names_the_host() {
    let own = std::process::id() as i32;
    let Some(host) = responsible_pid(own).filter(|host| *host != own) else {
        return;
    };

    let children = responsible_children(host);

    assert!(children.contains(&own));
    assert!(
        children
            .iter()
            .all(|child| responsible_pid(*child).is_none_or(|parent| parent == host))
    );
}
