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
pub(crate) mod shortcut_key;
pub(crate) mod shortcut_target;
pub(crate) mod window_overlay;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, KeyCombo, ProcessIdentity};
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
    let scan = match look_up(app, &target, lookup) {
        Ok(scan) => scan,
        Err(error) => return fallback_after(error, deadline, "lookup_incomplete"),
    };
    let mut matches = match scan {
        Some(MenuScan::Complete(matches)) => matches,
        Some(MenuScan::MenuOpen) => return Ok(MenuShortcutOutcome::Skipped("menu_open")),
        None => return Ok(MenuShortcutOutcome::Skipped("window_overlay")),
    };
    let enabled: Vec<bool> = matches.iter().map(|found| found.enabled).collect();
    let index = match MenuShortcutChoice::choose(&enabled) {
        MenuShortcutChoice::Press(index) => index,
        MenuShortcutChoice::Ambiguous => return Ok(MenuShortcutOutcome::Skipped("ambiguous")),
        MenuShortcutChoice::Disabled => return Ok(MenuShortcutOutcome::Skipped("disabled")),
        MenuShortcutChoice::NoMatch => return Ok(MenuShortcutOutcome::NotApplicable),
    };
    let found = matches.swap_remove(index);
    crate::system::process::process_identity::require_core(process)?;
    if !crate::actions::ax_helpers::try_ax_action_or_err(&found.element, "AXPress", deadline)? {
        return Ok(MenuShortcutOutcome::Skipped("not_pressable"));
    }
    Ok(MenuShortcutOutcome::Pressed {
        menu: found.menu,
        item: found.item,
    })
}

fn look_up(
    app: &AXElement,
    target: &ShortcutTarget,
    lookup: Deadline,
) -> Result<Option<MenuScan>, AdapterError> {
    let end = Instant::now()
        .checked_add(lookup.remaining())
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))?;
    if window_overlay::focused_window_has_overlay(app, end)? {
        return Ok(None);
    }
    menu_shortcut_walk::scan_menu_bar(app, target, lookup).map(Some)
}

/// The lookup is an optimization over the keyboard path, so any failure that
/// leaves the keyboard path able to run falls back to it. A permission failure
/// or an exhausted command deadline would fail that path too, so it surfaces.
fn fallback_after(
    error: AdapterError,
    deadline: Deadline,
    reason: &'static str,
) -> Result<MenuShortcutOutcome, AdapterError> {
    if error.code == ErrorCode::PermDenied {
        return Err(error);
    }
    deadline.ensure_remaining()?;
    tracing::debug!(code = ?error.code, "menu shortcut lookup fell back to key delivery");
    Ok(MenuShortcutOutcome::Skipped(reason))
}

#[cfg(test)]
#[path = "menu_shortcut_tests.rs"]
mod tests;
