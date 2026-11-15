use super::{
    focus_lost_before_delivery, press_key_from_gate, press_key_global, type_text_from_gate,
};
use crate::actions::keyboard_focus::KeyboardFocus;
use crate::input::keyboard_send_fake_sink as key_sink;
use crate::system::fixture::test_time::deadline;
use agent_desktop_core::{
    ActionStepOutcome, DeliveryDisposition, ErrorCode, KeyCombo, StepMechanism,
};

fn combo() -> KeyCombo {
    KeyCombo {
        key: "a".into(),
        modifiers: vec![],
    }
}

#[test]
fn type_text_lost_focus_fails_not_delivered_with_zero_injection() {
    key_sink::reset();

    let error =
        type_text_from_gate("hello", false, deadline(5_000), |_| Ok(())).expect_err("lost focus");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(key_sink::recorded().is_empty());
}

#[test]
fn type_text_with_focus_ready_synthesizes() {
    key_sink::reset();

    let step = type_text_from_gate("hi", true, deadline(5_000), |_| Ok(())).expect("type");

    assert_eq!(step.label(), "SendInput.type_text");
    assert_eq!(step.mechanism(), Some(StepMechanism::PhysicalSynthetic));
    assert_eq!(step.verified(), Some(false));
    assert!(matches!(step.outcome, ActionStepOutcome::Succeeded));
    assert!(!key_sink::recorded().is_empty());
}

#[test]
fn press_key_element_gate_synthesizes_when_focus_ready() {
    key_sink::reset();

    let step = press_key_from_gate(&combo(), true, deadline(5_000)).expect("press");

    assert_eq!(step.label(), "SendInput.press_key");
    assert_eq!(step.mechanism(), Some(StepMechanism::PhysicalSynthetic));
    assert!(!key_sink::recorded().is_empty());
}

#[test]
fn press_key_global_synthesizes_without_focus_gate() {
    key_sink::reset();

    let result = press_key_global(&combo(), deadline(5_000)).expect("global press");

    assert_eq!(result.action, "press");
    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.steps[0].label(), "SendInput.press_key");
    assert_eq!(
        result.steps[0].mechanism(),
        Some(StepMechanism::PhysicalSynthetic)
    );
    assert!(!key_sink::recorded().is_empty());
}

#[test]
fn press_key_lost_focus_fails_not_delivered_with_zero_injection() {
    key_sink::reset();

    let error = press_key_from_gate(&combo(), false, deadline(5_000)).expect_err("lost focus");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(key_sink::recorded().is_empty());
}

/// The per-chunk verifier is what stops a long `type` from continuing into
/// a window that took focus mid-sequence. Driving the shipped leg with a
/// verifier that fails after the first chunk proves the text path actually
/// consults it, which a fixed `|_| Ok(())` closure never could.
#[test]
fn type_text_stops_synthesizing_when_focus_is_lost_between_chunks() {
    use std::cell::Cell;
    key_sink::reset();
    let chunks_seen = Cell::new(0_u32);
    let long_text = "x".repeat(200);

    let error = type_text_from_gate(&long_text, true, deadline(5_000), |_| {
        chunks_seen.set(chunks_seen.get() + 1);
        if chunks_seen.get() > 1 {
            Err(focus_lost_before_delivery())
        } else {
            Ok(())
        }
    })
    .expect_err("focus lost between chunks must abort the type");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(
        chunks_seen.get() > 1,
        "the leg must re-verify between chunks, not only before the first"
    );
    let posted = key_sink::recorded().len();
    assert!(
        posted > 0 && posted < long_text.len(),
        "synthesis must stop part-way, not run to completion; posted {posted}"
    );
}

#[test]
fn a_focus_this_command_moved_is_reported_before_the_typed_text() {
    key_sink::reset();

    let keystroke = type_text_from_gate("hi", true, deadline(5_000), |_| Ok(())).expect("type");
    let steps = KeyboardFocus::FocusedBySetFocus.then(keystroke);

    let labels: Vec<&str> = steps.iter().map(|step| step.label()).collect();
    assert_eq!(labels, ["UIElement.SetFocus", "SendInput.type_text"]);
}

#[test]
fn a_focus_this_command_moved_is_reported_before_the_pressed_key() {
    key_sink::reset();

    let keystroke = press_key_from_gate(&combo(), true, deadline(5_000)).expect("press");
    let steps = KeyboardFocus::FocusedBySetFocus.then(keystroke);

    let labels: Vec<&str> = steps.iter().map(|step| step.label()).collect();
    assert_eq!(labels, ["UIElement.SetFocus", "SendInput.press_key"]);
}

#[test]
fn an_unfocused_focus_fallback_target_is_refused_with_no_keystroke() {
    key_sink::reset();

    let focus = KeyboardFocus::NotFocused;
    let error = type_text_from_gate("hi", focus.is_ready(), deadline(5_000), |_| Ok(()))
        .expect_err("refused");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(key_sink::recorded().is_empty());
}
