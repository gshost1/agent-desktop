//! Where each seeded wait entry's read is taken, decided once before the
//! first entry runs.

use crate::cli::Commands;

use super::{preparation::PreparedCommand, seed_kind::SeedKind};

/// For every action, the seeded wait entries whose reads are taken right
/// before it runs. A seeded wait is read before the nearest earlier entry
/// that can change what it observes, the action, so every wait after one
/// action measures from before that action, whatever sits between them. A
/// wait with no earlier action is not read for: it takes its own first read.
pub(super) struct SeedPlan {
    waits_read_before: Vec<Vec<usize>>,
}

impl SeedPlan {
    pub(super) fn new(commands: &[PreparedCommand]) -> Self {
        let mut waits_read_before = vec![Vec::new(); commands.len()];
        let mut last_input: Option<usize> = None;
        for (index, prepared) in commands.iter().enumerate() {
            if SeedKind::of(&prepared.command).is_some()
                && let Some(action) = last_input
            {
                waits_read_before[action].push(index);
            }
            if changes_what_waits_observe(&prepared.command) {
                last_input = Some(index);
            }
        }
        Self { waits_read_before }
    }

    /// The seeded wait entries whose reads are taken before entry `index`
    /// runs, in entry order.
    pub(super) fn waits_read_before(&self, index: usize) -> &[usize] {
        self.waits_read_before.get(index).map_or(&[], Vec::as_slice)
    }
}

/// Whether `command` can change what a seeded wait observes: windows, apps,
/// surfaces, notifications or focus. Every command that may send input to
/// an application counts, including app and window management, opening a
/// system surface, acting on a notification and a permission request,
/// except those that send nothing a wait watches for: clipboard writes,
/// cursor-overlay settings, session start and end, and dismissing
/// notifications. A wait after one of those still measures from the action
/// before it.
fn changes_what_waits_observe(command: &Commands) -> bool {
    command.effect().sends_input()
        && !matches!(
            command,
            Commands::ClipboardSet(_)
                | Commands::ClipboardClear
                | Commands::CursorOverlay(_)
                | Commands::Session(_)
                | Commands::DismissNotification(_)
                | Commands::DismissAllNotifications(_)
                | Commands::Batch(_)
        )
}

#[cfg(test)]
#[path = "seed_plan_tests.rs"]
mod tests;
