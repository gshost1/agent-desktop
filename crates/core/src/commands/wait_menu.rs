use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::{
    AppError, Deadline, MenuSeed, MenuWait, PlatformAdapter, ProcessIdentity,
    commands::{
        helpers::{process_identity, resolve_app},
        wait_timeout,
    },
    context::CommandContext,
};

/// The most a pre-action menu reading may spend before `batch` starts the
/// action without it. A seed only shortens the wait, so a read slower than
/// this is dropped, and each reading delays the action by at most this much;
/// two closed waits after one action take two readings.
const SEED_READ_CAP: Duration = Duration::from_millis(500);

pub(crate) fn wait_for_menu(
    app: Option<String>,
    open: bool,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let deadline = Deadline::at(start, timeout_ms)?;
    let target = resolve_app(app.as_deref(), adapter, deadline).map_err(|error| match error {
        AppError::Adapter(error) => wait_timeout::menu(error, timeout_ms),
        other => other,
    })?;
    let process = process_identity(&target)?;
    let grace = adapter.menu_opener_grace();
    let wait = menu_wait(open, &process, context.menu_seed(), start, grace);
    adapter
        .wait_for_menu(process.clone(), wait, deadline)
        .map_err(|error| {
            wait_timeout::menu(
                crate::ref_action::ref_action_wait_support::with_exited_process_state(
                    adapter, process, error, deadline,
                ),
                timeout_ms,
            )
        })?;
    let elapsed = start.elapsed().as_millis();
    Ok(json!({ "found": true, "elapsed_ms": elapsed }))
}

/// The phase a menu wait starts in. The open direction needs one open read.
/// The closed direction starts `Open` only when a reading of the same process
/// taken before the action saw the menu open; otherwise it awaits the menu
/// for the adapter's opener grace, because a closed read right after an
/// opener may be a menu that has not appeared yet.
pub(crate) fn menu_wait(
    open: bool,
    process: &ProcessIdentity,
    seed: Option<&MenuSeed>,
    started: Instant,
    grace: Duration,
) -> MenuWait {
    if open {
        return MenuWait::opened();
    }
    match seed {
        Some(seed) if seed.open && seed.process == *process => MenuWait::closed_after_open(),
        _ => MenuWait::closed_within(started, grace),
    }
}

/// Reads the menu state of the app a following `wait --menu-closed` targets,
/// before the action it waits after. Any failure leaves no seed, and the wait
/// then falls back to the opener grace.
pub fn menu_seed(
    app: Option<&str>,
    adapter: &dyn PlatformAdapter,
    deadline: Deadline,
) -> Option<MenuSeed> {
    let deadline = deadline.capped(SEED_READ_CAP);
    let target = resolve_app(app, adapter, deadline).ok()?;
    let process = process_identity(&target).ok()?;
    let open = adapter.menu_open(process.clone(), deadline).ok()?;
    Some(MenuSeed { process, open })
}

#[cfg(test)]
#[path = "wait_menu_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wait_menu_grace_tests.rs"]
mod grace_tests;
