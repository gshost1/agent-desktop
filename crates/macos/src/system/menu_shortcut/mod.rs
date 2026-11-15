//! Runs a command-key chord through the application's menu bar. A chord posted
//! as keystrokes lands only while the app is frontmost with the right window
//! key; pressing the menu item that publishes the chord as its shortcut runs
//! the same command without either, provided the item is enabled. The keyboard
//! path stays the fallback whenever the lookup cannot name exactly one enabled
//! item, except that a background app's disabled item is refused: its command
//! would not run, and neither would a keystroke to a background app.

pub(crate) mod background_chord;
pub(crate) mod menu_item_shortcut;
pub(crate) mod menu_match;
pub(crate) mod menu_node;
pub(crate) mod menu_scan;
pub(crate) mod menu_shortcut_choice;
pub(crate) mod menu_shortcut_outcome;
pub(crate) mod menu_shortcut_walk;
pub(crate) mod menu_walk;
pub(crate) mod shortcut_key;
pub(crate) mod shortcut_target;
pub(crate) mod window_overlay;

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, KeyCombo, ProcessIdentity,
};
use std::time::{Duration, Instant};

use crate::tree::AXElement;
use menu_scan::MenuScan;
use menu_shortcut_choice::MenuShortcutChoice;
use menu_shortcut_outcome::MenuShortcutOutcome;
use shortcut_target::ShortcutTarget;

const LOOKUP_BUDGET: Duration = Duration::from_millis(1_000);

pub(crate) const MENU_SHORTCUT_STEP: &str = "AXPress menu shortcut";

pub(crate) fn press_menu_shortcut(
    process: &ProcessIdentity,
    app: &AXElement,
    combo: &KeyCombo,
    deadline: Deadline,
) -> Result<MenuShortcutOutcome, AdapterError> {
    let Some(target) = ShortcutTarget::from_combo(combo) else {
        return Ok(MenuShortcutOutcome::NotApplicable);
    };
    let lookup = deadline.capped(LOOKUP_BUDGET);
    let end = Instant::now()
        .checked_add(lookup.remaining())
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))?;
    match window_overlay::focused_window_has_overlay(app, end) {
        Ok(false) => {}
        Ok(true) => return Ok(MenuShortcutOutcome::skipped("window_overlay")),
        Err(error) => return fallback_after(error, deadline, "overlay_check"),
    }
    let walk = match menu_shortcut_walk::scan_menu_bar(app, &target, lookup) {
        Ok(MenuScan::Complete(walk)) => walk,
        Ok(MenuScan::MenuOpen) => return Ok(MenuShortcutOutcome::skipped("menu_open")),
        Err(error) => return fallback_after(error, deadline, "menu_walk"),
    };
    let enabled: Vec<bool> = walk.matches.iter().map(|found| found.enabled).collect();
    let index = match MenuShortcutChoice::choose(&enabled) {
        MenuShortcutChoice::Press(index) => index,
        MenuShortcutChoice::Ambiguous => return Ok(skipped_after(&walk, "ambiguous")),
        MenuShortcutChoice::Disabled => {
            return only_disabled_matches(&walk, app_is_frontmost(app, end));
        }
        MenuShortcutChoice::NoMatch => return Ok(MenuShortcutOutcome::NotApplicable),
    };
    let Some(found) = walk.matches.get(index) else {
        return Ok(MenuShortcutOutcome::NotApplicable);
    };
    crate::system::process::process_identity::require_core(process)?;
    if !crate::actions::ax_helpers::try_ax_action_or_err(&found.element, "AXPress", deadline)? {
        return Ok(skipped_after(&walk, "not_pressable"));
    }
    let mut details = walk.report_fields();
    details.insert("menu".into(), serde_json::json!(found.menu));
    details.insert("item".into(), serde_json::json!(found.item));
    Ok(MenuShortcutOutcome::Pressed(serde_json::Value::Object(
        details,
    )))
}

/// AppKit enables menu items against the key window's responder chain, and an
/// application that is not frontmost has no key window. Its window commands
/// read as disabled there, and pressing one reports success while nothing runs.
fn app_is_frontmost(app: &AXElement, end: Instant) -> bool {
    crate::tree::surface_read::boolean(app, "AXFrontmost", end)
        .ok()
        .flatten()
        == Some(true)
}

/// A frontmost app's disabled item falls back to keys, which reach the key
/// window and may be handled there; a background app's is refused.
fn only_disabled_matches(
    walk: &menu_walk::MenuWalk,
    frontmost: bool,
) -> Result<MenuShortcutOutcome, AdapterError> {
    if frontmost {
        Ok(skipped_after(walk, "disabled"))
    } else {
        Err(disabled_in_background(walk))
    }
}

fn skipped_after(walk: &menu_walk::MenuWalk, reason: &'static str) -> MenuShortcutOutcome {
    let mut details = walk.report_fields();
    details.insert("skipped".into(), reason.into());
    MenuShortcutOutcome::Skipped(serde_json::Value::Object(details))
}

/// Refuses a chord whose only menu item is disabled in a background app. The
/// command would not run, and a keystroke posted to a background app does not
/// land either, so reporting either as delivered would be false.
fn disabled_in_background(walk: &menu_walk::MenuWalk) -> AdapterError {
    let mut menu_shortcut = walk.report_fields();
    let found = walk.matches.first();
    menu_shortcut.insert(
        "menu".into(),
        serde_json::json!(found.and_then(|found| found.menu.as_deref())),
    );
    menu_shortcut.insert(
        "item".into(),
        serde_json::json!(found.and_then(|found| found.item.as_deref())),
    );
    menu_shortcut.insert("enabled".into(), false.into());
    menu_shortcut.insert("frontmost".into(), false.into());
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Menu item for this shortcut is disabled while the app is in the background; nothing was sent",
    )
    .with_details(serde_json::json!({
        "kind": "menu_item_disabled_in_background",
        "menu_shortcut": menu_shortcut,
        "physical_delivery_started": false,
    }))
    .with_suggestion(
        "Retry with --headed, or bring the app's window forward with focus-window and press again; \
         the app enables window commands only while it is frontmost",
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

/// The lookup is an optimization over the keyboard path, so any failure that
/// leaves the keyboard path able to run falls back to it, and the report says
/// which stage failed and where. A permission failure or an exhausted command
/// deadline would fail that path too, so it surfaces.
fn fallback_after(
    error: AdapterError,
    deadline: Deadline,
    stage: &'static str,
) -> Result<MenuShortcutOutcome, AdapterError> {
    if error.code == ErrorCode::PermDenied {
        return Err(error);
    }
    deadline.ensure_remaining()?;
    tracing::debug!(code = ?error.code, stage, "menu shortcut lookup fell back to key delivery");
    let mut details = serde_json::json!({
        "skipped": "lookup_incomplete",
        "stage": stage,
        "code": error.code,
    });
    if let (Some(fields), Some(serde_json::Value::Object(extra))) =
        (details.as_object_mut(), error.details)
    {
        for key in [
            "kind",
            "menu",
            "depth",
            "visited",
            "unread_menus",
            "unread_after_budget",
            "slowest_read",
        ] {
            if let Some(value) = extra.get(key) {
                fields.insert(key.into(), value.clone());
            }
        }
    }
    Ok(MenuShortcutOutcome::Skipped(details))
}

#[cfg(test)]
#[path = "menu_shortcut_tests.rs"]
mod tests;
