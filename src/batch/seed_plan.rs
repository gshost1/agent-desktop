//! Where each seeded wait entry's pre-action read is taken, decided once
//! before the first entry runs.

use super::{preparation::PreparedCommand, seed_kind::SeedKind};

/// For every entry, the seeded wait entry whose read is taken right before
/// that entry runs, and the one whose read is taken again after it succeeds.
/// A seeded wait is read before the nearest earlier entry that can change the
/// desktop: one that sends input, or another seeded wait. Read-only and
/// local-write entries between them do not move the read after the action.
/// With no such entry, the read is taken before the entry right before the
/// wait. When that earlier entry is a seeded wait of the same kind, the read
/// is taken again after it succeeds, so the later wait does not report what
/// the earlier one already reported.
pub(super) struct SeedPlan {
    read_before: Vec<Option<usize>>,
    read_again_after: Vec<Option<usize>>,
}

impl SeedPlan {
    pub(super) fn new(commands: &[PreparedCommand]) -> Self {
        let kinds: Vec<Option<SeedKind>> = commands
            .iter()
            .map(|prepared| SeedKind::of(&prepared.command))
            .collect();
        let mut read_before = vec![None; commands.len()];
        let mut read_again_after = vec![None; commands.len()];
        let mut last_change = None;
        for (index, prepared) in commands.iter().enumerate() {
            if let Some(kind) = &kinds[index]
                && index > 0
            {
                let read_at = last_change.unwrap_or(index - 1);
                read_before[read_at] = Some(index);
                if kinds[read_at].as_ref() == Some(kind) {
                    read_again_after[read_at] = Some(index);
                }
            }
            if kinds[index].is_some() || prepared.command.effect().sends_input() {
                last_change = Some(index);
            }
        }
        Self {
            read_before,
            read_again_after,
        }
    }

    /// The seeded wait entry whose read is taken before entry `index` runs.
    pub(super) fn read_before(&self, index: usize) -> Option<usize> {
        self.read_before.get(index).copied().flatten()
    }

    /// The seeded wait entry whose read is taken again after entry `index`
    /// succeeds.
    pub(super) fn read_again_after(&self, index: usize) -> Option<usize> {
        self.read_again_after.get(index).copied().flatten()
    }
}

#[cfg(test)]
#[path = "seed_plan_tests.rs"]
mod tests;
