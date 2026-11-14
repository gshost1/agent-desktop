//! Change-only reads. `snapshot --since <snapshot_id>` takes a normal full
//! snapshot and reports only what differs from an earlier snapshot of the same
//! window in the same session: rows added, changed and removed, and the
//! focused element. Every full snapshot the `snapshot` command saves keeps a
//! baseline of its rows for this, beside its refmap.

pub(crate) mod changed_row;
pub(crate) mod diff_baseline;
pub(crate) mod diff_key;
pub mod diff_lines;
pub(crate) mod diff_match;
pub(crate) mod diff_owner;
pub(crate) mod diff_presentation;
pub(crate) mod diff_rows;
pub(crate) mod diff_status;
pub(crate) mod row_content;
pub(crate) mod snapshot_diff;
