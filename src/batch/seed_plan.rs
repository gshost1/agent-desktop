//! Where each seeded wait entry's pre-action read is taken, decided once
//! before the first entry runs.

use crate::cli::Commands;

use super::preparation::PreparedCommand;

/// For every entry, the seeded wait entry whose read is taken right before
/// that entry runs. A `wait` with `event` or `notification` is read before
/// the nearest earlier entry that can change the desktop: one that sends
/// input, or another seeded wait. Read-only and local-write entries between
/// them do not move the read after the action. With no such entry, the read
/// is taken before the entry right before the wait.
pub(super) struct SeedPlan {
    wait_read_before: Vec<Option<usize>>,
}

impl SeedPlan {
    pub(super) fn new(commands: &[PreparedCommand]) -> Self {
        let mut wait_read_before = vec![None; commands.len()];
        for (wait_index, wait) in commands.iter().enumerate() {
            if wait_index == 0 || !is_seeded_wait(&wait.command) {
                continue;
            }
            let read_before = (0..wait_index)
                .rev()
                .find(|&index| can_change_desktop(&commands[index].command))
                .unwrap_or(wait_index - 1);
            wait_read_before[read_before] = Some(wait_index);
        }
        Self { wait_read_before }
    }

    /// The seeded wait entry whose read is taken before entry `index` runs.
    pub(super) fn wait_read_before(&self, index: usize) -> Option<usize> {
        self.wait_read_before.get(index).copied().flatten()
    }
}

fn can_change_desktop(command: &Commands) -> bool {
    command.effect().sends_input() || is_seeded_wait(command)
}

fn is_seeded_wait(command: &Commands) -> bool {
    matches!(command, Commands::Wait(args) if args.mode.notification || args.event.event.is_some())
}

#[cfg(test)]
#[path = "seed_plan_tests.rs"]
mod tests;
