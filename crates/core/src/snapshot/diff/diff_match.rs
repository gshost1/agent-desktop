use std::collections::HashMap;

use crate::{
    AccessibilityNode,
    snapshot::SnapshotResult,
    snapshot::diff::{
        changed_row::ChangedRow,
        diff_baseline::DiffBaseline,
        diff_key::IdentityKey,
        diff_owner::DiffOwner,
        diff_presentation::DiffPresentation,
        diff_rows,
        diff_status::DiffStatus,
        row_content::{RowContent, changed_fields},
        snapshot_diff::SnapshotDiff,
    },
};

/// Compares the new snapshot `current` against `baseline`, the saved rows of
/// snapshot `since`.
///
/// A row pairs with an earlier row only through its identity, role plus the
/// stable fields a ref records, and only when that identity names exactly one
/// row on each side. A paired row whose visible fields differ is `changed`.
/// Every other row is matched by its whole content: an identical earlier row
/// cancels it, because nothing a reader sees differs, and anything left over is
/// `removed` or `added`. So a row whose identity cannot be matched is never
/// reported as changed.
pub(crate) fn compare(
    since: &str,
    baseline: &DiffBaseline,
    current: &SnapshotResult,
    presentation: DiffPresentation,
) -> SnapshotDiff {
    if baseline.owner != DiffOwner::of(&current.owner) {
        return SnapshotDiff::not_comparable(since, "owner_changed");
    }
    if baseline.presentation != presentation {
        return SnapshotDiff::not_comparable(since, "presentation_changed");
    }
    let rows = diff_rows::reported_rows(&current.tree);
    let mut diff = pair_rows(&baseline.rows, &rows);
    diff.since = since.to_owned();
    diff.focused = diff_rows::focused_row(&current.tree);
    diff.status = status(baseline.complete, current.complete, &diff);
    diff
}

fn pair_rows(before: &[AccessibilityNode], after: &[AccessibilityNode]) -> SnapshotDiff {
    let before_keys = key_counts(before);
    let after_keys = key_counts(after);
    let mut paired = vec![false; before.len()];
    let mut changed = Vec::new();
    let mut unpaired = Vec::new();
    for row in after {
        let key = IdentityKey::of(row);
        match (before_keys.get(&key), after_keys.get(&key)) {
            (Some(&(1, index)), Some(&(1, _))) => {
                paired[index] = true;
                let fields = changed_fields(&before[index], row);
                if !fields.is_empty() {
                    changed.push(ChangedRow {
                        now: row.clone(),
                        fields,
                        before: before[index].clone(),
                    });
                }
            }
            _ => unpaired.push(row),
        }
    }
    let leftover: Vec<&AccessibilityNode> = before
        .iter()
        .zip(&paired)
        .filter_map(|(row, paired)| (!paired).then_some(row))
        .collect();
    let (added, removed) = cancel_identical(&leftover, &unpaired);
    SnapshotDiff {
        since: String::new(),
        status: DiffStatus::Changed,
        added,
        changed,
        removed,
        focused: None,
    }
}

fn key_counts(rows: &[AccessibilityNode]) -> HashMap<IdentityKey<'_>, (usize, usize)> {
    let mut counts = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        let entry = counts.entry(IdentityKey::of(row)).or_insert((0, index));
        entry.0 += 1;
    }
    counts
}

fn cancel_identical(
    before: &[&AccessibilityNode],
    after: &[&AccessibilityNode],
) -> (Vec<AccessibilityNode>, Vec<AccessibilityNode>) {
    let mut spare: HashMap<RowContent<'_>, usize> = HashMap::new();
    for row in before {
        *spare.entry(RowContent::of(row)).or_default() += 1;
    }
    let mut cancelled: HashMap<RowContent<'_>, usize> = HashMap::new();
    let mut added = Vec::new();
    for row in after {
        let content = RowContent::of(row);
        match spare.get_mut(&content) {
            Some(count) if *count > 0 => {
                *count -= 1;
                *cancelled.entry(content).or_default() += 1;
            }
            _ => added.push((*row).clone()),
        }
    }
    let mut removed = Vec::new();
    for row in before {
        match cancelled.get_mut(&RowContent::of(row)) {
            Some(count) if *count > 0 => *count -= 1,
            _ => {
                let mut row = (*row).clone();
                row.ref_id = None;
                removed.push(row);
            }
        }
    }
    (added, removed)
}

fn status(baseline_complete: bool, snapshot_complete: bool, diff: &SnapshotDiff) -> DiffStatus {
    if !baseline_complete || !snapshot_complete {
        return DiffStatus::Incomplete {
            baseline: !baseline_complete,
            snapshot: !snapshot_complete,
        };
    }
    if diff.added.is_empty() && diff.changed.is_empty() && diff.removed.is_empty() {
        DiffStatus::Unchanged
    } else {
        DiffStatus::Changed
    }
}

#[cfg(test)]
#[path = "diff_match_tests.rs"]
mod tests;
