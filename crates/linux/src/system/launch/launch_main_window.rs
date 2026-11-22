pub(super) struct Candidate {
    pub(super) normal: Option<bool>,
    pub(super) transient: bool,
    pub(super) area: u32,
    pub(super) order: usize,
    pub(super) title: Option<String>,
}

pub(super) fn select_title(candidates: &[Candidate]) -> Option<String> {
    let largest = select_largest(candidates)?;
    let title = largest
        .title
        .as_ref()
        .filter(|title| !title.trim().is_empty())?;
    candidates
        .iter()
        .filter(|candidate| candidate.normal.unwrap_or(!candidate.transient))
        .filter(|candidate| candidate.area == largest.area)
        .all(|candidate| candidate.title.as_ref() == Some(title))
        .then(|| title.clone())
}

fn select_largest(candidates: &[Candidate]) -> Option<&Candidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.normal.unwrap_or(!candidate.transient))
        .max_by_key(|candidate| (candidate.area, candidate.order))
}

#[cfg(test)]
#[path = "launch_main_window_tests.rs"]
mod tests;
