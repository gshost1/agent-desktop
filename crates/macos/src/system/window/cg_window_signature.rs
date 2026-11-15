//! The comparable shape of one window inventory capture. Two consecutive
//! captures with the same signature are the stable inventory.

use crate::system::window::cg_window::WindowRecord;
use crate::system::window::window_presence::WindowPresence;

pub(super) type InventorySignatureEntry<'a> = (
    i32,
    i64,
    &'a str,
    Option<&'a str>,
    [u64; 4],
    WindowPresence,
    &'a str,
);

pub(super) fn inventory_signature(records: &[WindowRecord]) -> Vec<InventorySignatureEntry<'_>> {
    let mut signature = records
        .iter()
        .map(|record| {
            (
                record.pid,
                record.window_number,
                record.app_name.as_str(),
                record.title.as_deref(),
                [
                    record.bounds.x.to_bits(),
                    record.bounds.y.to_bits(),
                    record.bounds.width.to_bits(),
                    record.bounds.height.to_bits(),
                ],
                record.presence,
                record.process_instance.as_deref().unwrap_or(""),
            )
        })
        .collect::<Vec<_>>();
    signature.sort_unstable();
    signature
}
