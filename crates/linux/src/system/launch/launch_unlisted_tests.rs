use super::*;

fn suggestion_for(seen: OffBus) -> String {
    report_unlisted("adl-absent", u32::MAX, seen)
        .notes
        .suggestion
        .expect("an unlisted launch always suggests a next step")
}

#[test]
fn a_process_still_starting_is_told_to_wait_for_its_window() {
    let text = suggestion_for(OffBus::Starting);
    assert!(text.contains("still starting"), "{text}");
    assert!(
        text.contains("agent-desktop wait --app 'adl-absent' --window"),
        "{text}"
    );
    assert!(!text.contains("toolkit may need"), "{text}");
}

#[test]
fn only_a_window_without_bus_presence_blames_the_toolkit() {
    let text = suggestion_for(OffBus::Windowed);
    assert!(text.contains("shows a window"), "{text}");
    assert!(text.contains("toolkit may need a flag"), "{text}");
    assert!(!text.contains("still starting"), "{text}");
}

#[test]
fn a_bridge_turned_off_names_the_setting() {
    let text = suggestion_for(OffBus::BridgeOff);
    assert!(text.contains("NO_AT_BRIDGE=1"), "{text}");
    assert!(!text.contains("still starting"), "{text}");
}

#[test]
fn an_unreadable_process_is_reported_under_the_requested_name() {
    let result = report_unlisted("adl-absent", u32::MAX, OffBus::Starting);
    assert_eq!(result.app, "adl-absent");
    assert!(result.window.is_none());
    assert!(result.process_instance.is_none());
}
