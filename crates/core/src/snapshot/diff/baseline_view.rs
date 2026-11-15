use serde::Serialize;

use crate::snapshot::diff::{
    diff_owner::DiffOwner, diff_presentation::DiffPresentation, row_view::RowSeq,
};

/// A baseline as it is written: the same fields `DiffBaseline` reads back,
/// with the rows borrowed from the snapshot's tree.
#[derive(Serialize)]
pub(crate) struct BaselineView<'a> {
    pub(crate) format: u32,
    pub(crate) owner: DiffOwner,
    pub(crate) presentation: DiffPresentation,
    pub(crate) complete: bool,
    pub(crate) rows: RowSeq<'a>,
}
