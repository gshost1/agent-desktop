/// The input a fill assigned its value to whole, because the input's type
/// takes no typed text: a date, time, datetime-local, month, week, color or
/// range input. `unchanged` means the input already held the value, so
/// nothing was assigned and no event fired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedInput {
    pub input_type: String,
    pub unchanged: bool,
}
