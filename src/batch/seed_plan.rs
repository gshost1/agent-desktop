//! Where each seeded wait entry's read is taken, decided once before the
//! first entry runs.

use super::{preparation::PreparedCommand, read_point::ReadPoint, seed_kind::SeedKind};

/// For every entry, the read it owes a later seeded wait. A seeded wait is
/// read at the nearest earlier entry that can change the desktop: one that
/// sends input, or another seeded wait. Read-only and local-write entries
/// between them do not move the read after the action. With no such entry,
/// the read is taken at the entry right before the wait. The read is taken
/// before that entry runs, except when the entry is a seeded wait with the
/// same filter: then it is taken once that wait finishes, so the later wait
/// does not report what the earlier one already reported. Each seeded wait
/// after the first entry gets exactly one read.
pub(super) struct SeedPlan {
    reads: Vec<Option<ReadPoint>>,
}

impl SeedPlan {
    pub(super) fn new(commands: &[PreparedCommand]) -> Self {
        let kinds: Vec<Option<SeedKind>> = commands
            .iter()
            .map(|prepared| SeedKind::of(&prepared.command))
            .collect();
        let mut reads = vec![None; commands.len()];
        let mut last_change = None;
        for (index, prepared) in commands.iter().enumerate() {
            if let Some(kind) = &kinds[index]
                && index > 0
            {
                let read_at = last_change.unwrap_or(index - 1);
                reads[read_at] = Some(if kinds[read_at].as_ref() == Some(kind) {
                    ReadPoint::After { wait_index: index }
                } else {
                    ReadPoint::Before { wait_index: index }
                });
            }
            if kinds[index].is_some() || prepared.command.effect().sends_input() {
                last_change = Some(index);
            }
        }
        Self { reads }
    }

    /// The read entry `index` owes a later seeded wait.
    pub(super) fn read_at(&self, index: usize) -> Option<ReadPoint> {
        self.reads.get(index).copied().flatten()
    }
}

#[cfg(test)]
#[path = "seed_plan_tests.rs"]
mod tests;
