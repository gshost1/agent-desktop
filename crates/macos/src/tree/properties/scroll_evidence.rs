pub(crate) fn vertical_overflow(
    bar_present: Option<bool>,
    enabled: Result<Option<bool>, i32>,
) -> Option<bool> {
    match bar_present {
        Some(true) => enabled.ok().flatten(),
        Some(false) | None => None,
    }
}

#[cfg(test)]
#[path = "scroll_evidence_tests.rs"]
mod tests;
