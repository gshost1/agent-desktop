//! Runs a command-key chord through the application's menu bar. A chord posted
//! as keystrokes lands only while the app is frontmost with the right window
//! key; pressing the menu item that publishes the chord as its shortcut runs
//! the same command without either. The keyboard path stays the fallback
//! whenever the lookup cannot name exactly one enabled item.

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

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, KeyCombo, ProcessIdentity};
use std::time::{Duration, Instant};

use crate::tree::AXElement;
use menu_match::MenuMatch;
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
    let enabled_is_current = app_is_frontmost(app, end);
    let walk = match menu_shortcut_walk::scan_menu_bar(app, &target, enabled_is_current, lookup) {
        Ok(MenuScan::Complete(walk)) => walk,
        Ok(MenuScan::MenuOpen) => return Ok(MenuShortcutOutcome::skipped("menu_open")),
        Err(error) => return fallback_after(error, deadline, "menu_walk"),
    };
    let pressable = pressable_flags(&walk.matches, enabled_is_current);
    let mut matches = walk.matches;
    let index = match MenuShortcutChoice::choose(&pressable) {
        MenuShortcutChoice::Press(index) => index,
        MenuShortcutChoice::Ambiguous => return Ok(MenuShortcutOutcome::skipped("ambiguous")),
        MenuShortcutChoice::Disabled => return Ok(MenuShortcutOutcome::skipped("disabled")),
        MenuShortcutChoice::NoMatch => return Ok(MenuShortcutOutcome::NotApplicable),
    };
    let found = matches.swap_remove(index);
    crate::system::process::process_identity::require_core(process)?;
    if !crate::actions::ax_helpers::try_ax_action_or_err(&found.element, "AXPress", deadline)? {
        return Ok(MenuShortcutOutcome::skipped("not_pressable"));
    }
    Ok(MenuShortcutOutcome::Pressed(pressed_details(
        &found,
        enabled_is_current,
        &walk.unread_menus,
    )))
}

/// AppKit validates menu items against the key window's responder chain, and
/// an application that is not frontmost has no key window, so its window
/// commands read as disabled whether or not they would run. Only the
/// frontmost application's `AXEnabled` is a current answer.
fn app_is_frontmost(app: &AXElement, end: Instant) -> bool {
    crate::tree::surface_read::boolean(app, "AXFrontmost", end)
        .ok()
        .flatten()
        == Some(true)
}

fn pressable_flags(matches: &[MenuMatch], enabled_is_current: bool) -> Vec<bool> {
    matches
        .iter()
        .map(|found| found.enabled || !enabled_is_current)
        .collect()
}

fn pressed_details(
    found: &MenuMatch,
    enabled_is_current: bool,
    unread_menus: &[String],
) -> serde_json::Value {
    let mut details = serde_json::json!({
        "menu": found.menu,
        "item": found.item,
        "enabled": if enabled_is_current { serde_json::json!(found.enabled) } else { serde_json::json!("unvalidated") },
    });
    if !unread_menus.is_empty()
        && let Some(fields) = details.as_object_mut()
    {
        fields.insert("unread_menus".into(), serde_json::json!(unread_menus));
    }
    details
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
        for key in ["kind", "menu", "depth", "visited"] {
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
