//! Where each seeded wait entry's read is taken, decided once before the
//! first entry runs.

use super::{preparation::PreparedCommand, seed_kind::SeedKind};

/// For every entry that sends input, the seeded wait entries whose reads are
/// taken right before it runs. A seeded wait is read before the nearest
/// earlier entry that sends input, the action it observes, so every wait
/// after one action measures from before that action, whatever sits between
/// them. A wait with no earlier input entry is not read for: it takes its own
/// first read.
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
            if prepared.command.effect().sends_input() {
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

#[cfg(test)]
#[path = "seed_plan_tests.rs"]
mod tests;
