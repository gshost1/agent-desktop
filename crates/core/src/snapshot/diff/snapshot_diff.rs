use serde_json::{Value, json};

use crate::{
    AccessibilityNode, AppError,
    snapshot::diff::{changed_row::ChangedRow, diff_status::DiffStatus},
};

/// The change-only reading of a new snapshot against an earlier one in the
/// same session: rows that appeared, rows whose visible fields changed, rows
/// that disappeared, and the element that holds keyboard focus now.
#[derive(Debug, Clone)]
pub(crate) struct SnapshotDiff {
    pub(crate) since: String,
    pub(crate) status: DiffStatus,
    pub(crate) added: Vec<AccessibilityNode>,
    pub(crate) changed: Vec<ChangedRow>,
    pub(crate) removed: Vec<AccessibilityNode>,
    pub(crate) focused: Option<AccessibilityNode>,
}

impl SnapshotDiff {
    pub(crate) fn not_comparable(since: &str, reason: &'static str) -> Self {
        Self {
            since: since.to_owned(),
            status: DiffStatus::NotComparable(reason),
            added: Vec::new(),
            changed: Vec::new(),
            removed: Vec::new(),
            focused: None,
        }
    }

    pub(crate) fn is_comparable(&self) -> bool {
        !matches!(self.status, DiffStatus::NotComparable(_))
    }

    /// `data.diff` as the JSON contract states it. Empty row lists are left
    /// out; `incomplete` names the side or sides that were cut short and
    /// `reason` says why two snapshots could not be compared.
    pub(crate) fn to_json(&self) -> Result<Value, AppError> {
        let mut diff = json!({ "since": self.since, "status": self.status.as_str() });
        match self.status {
            DiffStatus::Incomplete { baseline, snapshot } => {
                let sides: Vec<&str> = [(baseline, "baseline"), (snapshot, "snapshot")]
                    .into_iter()
                    .filter_map(|(cut, side)| cut.then_some(side))
                    .collect();
                diff["incomplete"] = json!(sides);
            }
            DiffStatus::NotComparable(reason) => diff["reason"] = json!(reason),
            DiffStatus::Unchanged | DiffStatus::Changed => {}
        }
        if !self.added.is_empty() {
            diff["added"] = serde_json::to_value(&self.added)?;
        }
        if !self.changed.is_empty() {
            let rows = self
                .changed
                .iter()
                .map(ChangedRow::to_json)
                .collect::<Result<Vec<_>, _>>()?;
            diff["changed"] = Value::Array(rows);
        }
        if !self.removed.is_empty() {
            diff["removed"] = serde_json::to_value(&self.removed)?;
        }
        if let Some(focused) = &self.focused {
            diff["focused"] = serde_json::to_value(focused)?;
        }
        Ok(diff)
    }
}
