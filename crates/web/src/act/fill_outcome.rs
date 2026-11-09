use agent_desktop_core::DeliverySemantics;

use super::assigned_input::AssignedInput;

/// What a fill left in the element: the text read back after the edit, and
/// whether it equals the requested text. A page may reformat, truncate
/// (`maxlength`) or reject input, so `verified` can be false after a
/// delivered edit. `assigned` is set when the element is an input that
/// takes its value whole, such as a date or time input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillOutcome {
    pub observed: Option<String>,
    pub verified: bool,
    pub assigned: Option<AssignedInput>,
}

impl FillOutcome {
    /// `not_delivered` when the input already held the value,
    /// `delivered_verified` when the read-back matched, otherwise
    /// `delivered_unverified`.
    pub fn delivery(&self) -> DeliverySemantics {
        if self.assigned.as_ref().is_some_and(|input| input.unchanged) {
            DeliverySemantics::not_delivered()
        } else if self.verified {
            DeliverySemantics::delivered_verified()
        } else {
            DeliverySemantics::delivered_unverified()
        }
    }
}
