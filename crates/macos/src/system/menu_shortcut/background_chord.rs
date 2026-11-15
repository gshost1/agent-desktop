//! A command chord on a character key reaches an AppKit application through
//! its menu bar's key equivalents, and an application that is not frontmost
//! dispatches none of them: the keystroke is accepted by the window server and
//! no command runs. Measured on a background document editor, `cmd+b` and
//! `cmd+s` posted to its process changed neither the selection's style nor the
//! file, while `cmd+right`, `cmd+up` and `alt+shift+down` still moved the
//! selection. So when the menu lookup pressed nothing, a character-key command
//! chord for a background application is refused, not reported as sent.

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, KeyCombo};

use super::menu_shortcut_outcome::MenuShortcutOutcome;
use super::shortcut_key::ShortcutKey;
use super::shortcut_target::ShortcutTarget;

/// Whether `combo` is a command chord on a character key, the kind only a
/// menu key equivalent handles.
pub(crate) fn runs_only_through_menu(combo: &KeyCombo) -> bool {
    matches!(
        ShortcutTarget::from_combo(combo).map(|target| target.key),
        Some(ShortcutKey::Character(_))
    )
}

/// Whether the application reports itself as not frontmost. An unreadable
/// answer is not a background reading, so the keyboard path keeps running.
#[cfg(target_os = "macos")]
pub(crate) fn reads_background(
    app: &crate::tree::AXElement,
    deadline: agent_desktop_core::Deadline,
) -> Result<bool, AdapterError> {
    let end = crate::tree::locator_deadline::from_operation(deadline)?;
    Ok(matches!(
        crate::tree::surface_read::boolean(app, "AXFrontmost", end),
        Ok(Some(false))
    ))
}

/// The refusal for a character-key command chord that no enabled menu item of
/// a background application took.
pub(crate) fn unrouted_in_background(combo: &KeyCombo, menu: &MenuShortcutOutcome) -> AdapterError {
    let lookup = match menu {
        MenuShortcutOutcome::Skipped(details) => details.clone(),
        MenuShortcutOutcome::Pressed(details) => details.clone(),
        MenuShortcutOutcome::NotApplicable => serde_json::json!({ "matches": 0 }),
    };
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "No enabled menu item publishes {}, and the app is in the background, where a command keystroke runs nothing; nothing was sent",
            chord_text(combo)
        ),
    )
    .with_details(serde_json::json!({
        "kind": "command_chord_unrouted_in_background",
        "menu_shortcut": lookup,
        "frontmost": false,
        "physical_delivery_started": false,
    }))
    .with_suggestion(
        "Retry with --headed so the app comes to the front, or click the command's menu item: \
         'find --app <app> --surface menubar --name <item>' then 'click <ref>'.",
    )
    .with_disposition(DeliverySemantics::not_delivered())
}

fn chord_text(combo: &KeyCombo) -> String {
    let mut parts = combo
        .modifiers
        .iter()
        .map(|modifier| match modifier {
            agent_desktop_core::Modifier::Meta => "cmd",
            agent_desktop_core::Modifier::Shift => "shift",
            agent_desktop_core::Modifier::Alt => "alt",
            agent_desktop_core::Modifier::Ctrl => "ctrl",
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    parts.push(combo.key.clone());
    parts.join("+")
}

#[cfg(test)]
#[path = "background_chord_tests.rs"]
mod tests;
