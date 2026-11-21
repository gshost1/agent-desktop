use super::*;

#[test]
fn the_linux_skill_explains_unread_popup_closure_and_escape_delivery() {
    let result = get(GetArgs {
        name: "linux".into(),
        full: false,
        reference: Some("shell-tray-and-notifications".into()),
    })
    .expect("linux popup guidance");
    let content = result["content"].as_str().expect("content");
    assert!(content.contains("`null` when its closed state could not be read"));
    assert!(
        content.contains("`details.popup_close_attempted` is `true` only when Escape was sent")
    );
}

fn macos_skill() -> String {
    let result = get(GetArgs {
        name: "macos".into(),
        full: false,
        reference: None,
    })
    .expect("macos skill");
    result["content"].as_str().expect("content").to_string()
}

#[test]
fn the_macos_skill_filters_long_lists_and_acts_on_the_filtered_refs() {
    let content = macos_skill();
    assert!(content.contains("grep -i"), "{content}");
    assert!(content.contains("do not type the text again"), "{content}");
    assert!(
        !content.contains("`set-value` its name into the window's"),
        "{content}"
    );
}

#[test]
fn the_macos_skill_never_names_headed_as_the_first_retry() {
    let content = macos_skill();
    assert!(!content.contains("Rerun with `--headed`"), "{content}");
    assert!(!content.contains("needs physical input"), "{content}");
    assert!(content.contains("Use `--headed` instead only if the user allows physical input"));
}
