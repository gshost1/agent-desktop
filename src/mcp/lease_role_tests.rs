use super::*;

fn role(globals: &[&str], argv: &[&str]) -> LeaseRole {
    let words = |words: &[&str]| words.iter().map(ToString::to_string).collect::<Vec<_>>();
    LeaseRole::of_call(&words(globals), &words(argv))
}

#[test]
fn input_and_focus_are_exclusive_reads_are_shared_and_lease_free_commands_get_nothing() {
    for argv in [
        &["permissions", "--request"][..],
        &["click", "--", "@s8f3k2p9:e1"],
        &["press", "--", "ctrl+c"],
        &["clipboard-get", "--out=/tmp/x"],
        &["no-such-command"],
    ] {
        assert_eq!(role(&[], argv), LeaseRole::Exclusive, "{argv:?}");
    }
    for argv in [&["list-notifications"][..], &["wait", "--notification"]] {
        assert_eq!(role(&["--headed"], argv), LeaseRole::Exclusive, "{argv:?}");
        assert_eq!(role(&[], argv), LeaseRole::Shared, "{argv:?}");
    }
    for argv in [
        &["snapshot"][..],
        &["find", "--role=button"],
        &["wait", "--text=Done"],
        &["screenshot"],
        &["list-surfaces"],
    ] {
        assert_eq!(role(&["--headed"], argv), LeaseRole::Shared, "{argv:?}");
    }
    for argv in [
        &["list-apps"][..],
        &["list-windows"],
        &["list-displays"],
        &["version"],
        &["skills", "list"],
        &["clipboard-get"],
    ] {
        assert_eq!(role(&[], argv), LeaseRole::Unused, "{argv:?}");
    }
}

#[test]
fn status_and_permissions_share_the_lease_like_other_permission_reads() {
    for argv in [&["status"][..], &["permissions"]] {
        assert_eq!(role(&[], argv), LeaseRole::Shared, "{argv:?}");
        assert_eq!(role(&["--headed"], argv), LeaseRole::Shared, "{argv:?}");
    }
}
