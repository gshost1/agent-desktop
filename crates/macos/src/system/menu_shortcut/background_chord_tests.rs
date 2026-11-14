use super::{runs_only_through_menu, unrouted_in_background};
use crate::system::menu_shortcut::menu_shortcut_outcome::MenuShortcutOutcome;
use agent_desktop_core::{DeliverySemantics, ErrorCode, KeyCombo, Modifier};

fn chord(key: &str, modifiers: &[Modifier]) -> KeyCombo {
    KeyCombo {
        key: key.into(),
        modifiers: modifiers.to_vec(),
    }
}

#[test]
fn command_chords_on_character_keys_run_only_through_the_menu() {
    assert!(runs_only_through_menu(&chord("s", &[Modifier::Meta])));
    assert!(runs_only_through_menu(&chord(
        "t",
        &[Modifier::Meta, Modifier::Shift]
    )));
    assert!(runs_only_through_menu(&chord("comma", &[Modifier::Meta])));
    assert!(runs_only_through_menu(&chord(
        "5",
        &[Modifier::Meta, Modifier::Alt]
    )));
}

#[test]
fn navigation_chords_and_plain_keys_still_reach_the_focused_view() {
    for key in [
        "right", "up", "left", "down", "home", "end", "delete", "tab", "return",
    ] {
        assert!(
            !runs_only_through_menu(&chord(key, &[Modifier::Meta])),
            "{key}"
        );
    }
    assert!(!runs_only_through_menu(&chord("s", &[])));
    assert!(!runs_only_through_menu(&chord("b", &[Modifier::Ctrl])));
    assert!(!runs_only_through_menu(&chord(
        "down",
        &[Modifier::Alt, Modifier::Shift]
    )));
}

#[test]
fn the_refusal_reports_nothing_sent_and_carries_the_lookup() {
    let lookup = MenuShortcutOutcome::Skipped(serde_json::json!({ "skipped": "ambiguous" }));

    let error = unrouted_in_background(&chord("s", &[Modifier::Meta]), &lookup);

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.message.contains("cmd+s"));
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "command_chord_unrouted_in_background");
    assert_eq!(details["menu_shortcut"]["skipped"], "ambiguous");
    assert_eq!(details["physical_delivery_started"], false);
}
