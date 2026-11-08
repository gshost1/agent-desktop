//! The process a press reaches, and which routes may act on it. A proven
//! hosted view, such as a Save panel that another process serves inside the
//! application's sheet, owns the keyboard: its controls do not belong to the
//! application, so neither the application's semantic actions nor a sheet
//! postcondition read through the application describe what the key did.

use agent_desktop_core::{ActionResult, AdapterError, Deadline, KeyCombo, ProcessIdentity};

use crate::system::window::hosted_focus_effect::merge_detail;
use crate::system::window::hosted_focus_read::HostedFocus;
use crate::tree::AXElement;

/// The application a press targets, with the hosted focus evidence read for
/// it. Every real key of a press, on every route, goes to `delivery_pid`.
pub(super) struct KeyTarget<'a> {
    pub(super) process: &'a ProcessIdentity,
    pub(super) app: &'a AXElement,
    pub(super) hosted: Option<&'a HostedFocus>,
}

impl KeyTarget<'_> {
    pub(super) fn host_owns_keys(&self) -> bool {
        !self.hosted.is_some_and(HostedFocus::is_hosted_view)
    }

    pub(super) fn delivery_pid(&self, deadline: Deadline) -> Result<i32, AdapterError> {
        match self.hosted {
            Some(focus) => focus.delivery_pid(self.app, deadline),
            None => Ok(crate::system::process::process_identity::require_core(self.process)?.pid()),
        }
    }

    /// Adds `keyboard_target` beside the details the route reported.
    pub(super) fn report(&self, mut result: ActionResult) -> ActionResult {
        if let Some(target) = self.hosted.and_then(HostedFocus::details) {
            result.details = Some(merge_detail(result.details, "keyboard_target", target));
        }
        result
    }

    pub(super) fn annotate(&self, mut error: AdapterError) -> AdapterError {
        if let Some(target) = self.hosted.and_then(HostedFocus::details) {
            error.details = Some(merge_detail(error.details, "keyboard_target", target));
        }
        error
    }
}

/// The semantic action behind a confirm or cancel key, for the routes that
/// act through the application's own accessibility: the sheet or dialog
/// route and the plain semantic action. Neither applies while a proven hosted
/// view owns the keys; the key then goes to the hosted view's process and is
/// verified by the hosted focus effect.
pub(super) fn host_key_action(host_owns_keys: bool, combo: &KeyCombo) -> Option<&'static str> {
    if !host_owns_keys {
        return None;
    }
    super::simple_key_action(combo)
}

#[cfg(test)]
#[path = "key_dispatch_target_tests.rs"]
mod tests;
