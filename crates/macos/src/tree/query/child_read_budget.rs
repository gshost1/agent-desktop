#[derive(Clone, Copy)]
pub(crate) struct ChildReadBudget {
    pub(crate) max_elements: usize,
    pub(crate) boundary: bool,
}

impl ChildReadBudget {
    pub(crate) fn inferred(max_elements: usize) -> Self {
        Self {
            max_elements,
            boundary: max_elements == 0,
        }
    }
}

impl From<usize> for ChildReadBudget {
    fn from(max_elements: usize) -> Self {
        Self::inferred(max_elements)
    }
}

/// A boundary node is read for its child count alone, purely to annotate how
/// much was left unobserved. On a renderer that materialises children lazily
/// that count is not cheap — asking for it can cost more than the traversal it
/// was describing, and a single one could consume an entire snapshot budget.
/// The count is therefore best-effort: it gets a small slice, and a boundary
/// that cannot afford one is still reported as truncated, just without a number.
const BOUNDARY_COUNT_BUDGET: std::time::Duration = std::time::Duration::from_millis(25);

/// The same reasoning applies when the children are about to be read: a count
/// that consumes the whole deadline starves the read it was meant to size, so
/// it never gets more than half of what is left.
pub(super) fn count_deadline(boundary: bool, deadline: std::time::Instant) -> std::time::Instant {
    let now = std::time::Instant::now();
    let budget = if boundary {
        BOUNDARY_COUNT_BUDGET
    } else {
        deadline.saturating_duration_since(now) / 2
    };
    deadline.min(now + budget)
}

/// A second count after the prefix read proves the children did not change
/// while they were being read. With nothing read there is no read for the
/// count to bracket: a second count would only sample the same moment again,
/// so a leaf costs one count instead of two.
pub(super) fn recount_after_prefix(requested: usize) -> bool {
    requested > 0
}

/// The page length that reads every child and proves the count in one call:
/// one more than the count, so a list that grew since the count shows up as
/// an extra element instead of being cut silently. `None` when the read is cut
/// by the element budget or spans pages, where the second count stays.
pub(super) fn whole_list_page(
    count: usize,
    max_elements: usize,
    page_size: usize,
) -> Option<usize> {
    (count > 0 && count <= max_elements && count < page_size).then_some(count + 1)
}

/// Reads a counted list and says whether the read itself proved the count.
/// Only a page that answered the longer `whole_page` range proves it: when the
/// responder refuses that range, the exact-range read that stands in stops at
/// the count and cannot see a child added since, so it is never proof and the
/// second count stays.
pub(super) fn read_counted_list<T, S>(
    state: &mut S,
    count: usize,
    whole_page: Option<usize>,
    read_whole: impl FnOnce(&mut S, usize) -> Option<Result<Vec<T>, i32>>,
    read_exact: impl FnOnce(&mut S) -> Result<Vec<T>, i32>,
) -> (Result<Vec<T>, i32>, bool) {
    match whole_page.and_then(|page_len| read_whole(state, page_len)) {
        Some(read) => {
            let proven = read.as_ref().is_ok_and(|elements| elements.len() == count);
            (read, proven)
        }
        None => (read_exact(state), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_whole_list_on_one_page_asks_for_one_child_more_than_the_count() {
        assert_eq!(whole_list_page(3, 4_096, 128), Some(4));
        assert_eq!(whole_list_page(127, 4_096, 128), Some(128));
        assert_eq!(whole_list_page(5, 5, 128), Some(6));
    }

    #[test]
    fn a_cut_multi_page_or_empty_list_keeps_the_second_count() {
        assert_eq!(whole_list_page(0, 4_096, 128), None);
        assert_eq!(
            whole_list_page(6, 5, 128),
            None,
            "a read cut by its budget cannot see the tail, so only a count proves it"
        );
        assert_eq!(whole_list_page(128, 4_096, 128), None);
        assert_eq!(whole_list_page(500, 4_096, 128), None);
    }

    #[test]
    fn only_a_read_prefix_is_bracketed_by_a_second_count() {
        assert!(!recount_after_prefix(0));
        assert!(recount_after_prefix(1));
        assert!(recount_after_prefix(128));
    }

    #[test]
    fn a_boundary_count_gets_only_a_slice_of_a_long_deadline() {
        let far = Instant::now() + Duration::from_secs(3);

        let bounded = count_deadline(true, far);

        assert!(
            bounded < far,
            "a boundary node is read for its count alone; letting one lazy renderer container \
             spend the whole snapshot budget on it is what this bound exists to prevent"
        );
        assert!(bounded <= Instant::now() + Duration::from_millis(60));
    }

    /// A slow responder that cannot answer a count must still leave the caller
    /// enough budget to read the values themselves.
    #[test]
    fn a_real_child_read_leaves_half_its_deadline_for_the_values() {
        let far = Instant::now() + Duration::from_secs(3);

        let bounded = count_deadline(false, far);
        let after = Instant::now();

        assert!(bounded < far);
        assert!(
            far.saturating_duration_since(bounded) >= far.saturating_duration_since(after) / 2,
            "the count may spend at most half of what was left when it was bounded"
        );
    }

    #[test]
    fn the_bound_never_extends_a_deadline_that_is_already_shorter() {
        let near = Instant::now() + Duration::from_millis(5);

        assert!(count_deadline(true, near) <= near);
    }

    #[test]
    fn a_boundary_requesting_label_children_still_gets_the_25ms_count_budget() {
        let far = Instant::now() + Duration::from_secs(3);
        let budget = super::super::child_read_plan::ChildReadPlan::boundary_aware(128, 5, 3, 3)
            .max_elements(false, true);
        assert_eq!(budget.max_elements, 5);

        let earliest = Instant::now() + BOUNDARY_COUNT_BUDGET;
        let bounded = count_deadline(budget.boundary, far);
        let latest = Instant::now() + BOUNDARY_COUNT_BUDGET;

        assert!(
            earliest <= bounded && bounded <= latest,
            "requesting a handful of label children at a boundary must keep the 25 ms count \
             slice, not fall back to the generous half-deadline budget"
        );
    }

    #[test]
    fn an_exact_range_read_after_a_refused_whole_page_is_never_proof() {
        let mut exact_reads = 0;

        let (read, proven) = read_counted_list(
            &mut exact_reads,
            3,
            whole_list_page(3, 4_096, 128),
            |_, _| None::<Result<Vec<u8>, i32>>,
            |reads| {
                *reads += 1;
                Ok(vec![0, 1, 2])
            },
        );

        assert_eq!(read, Ok(vec![0, 1, 2]));
        assert_eq!(exact_reads, 1);
        assert!(
            !proven,
            "an exact-range read stops at the count, so a child added since the count is \
             invisible to it; only the second count can prove the list did not grow"
        );
    }

    #[test]
    fn only_a_whole_page_holding_exactly_the_count_proves_it() {
        let whole = |page: Result<Vec<u8>, i32>| {
            read_counted_list(&mut (), 3, Some(4), |_, _| Some(page), |_| Ok(Vec::new())).1
        };

        assert!(whole(Ok(vec![0, 1, 2])));
        assert!(
            !whole(Ok(vec![0, 1, 2, 3])),
            "a grown list shows its extra child"
        );
        assert!(
            !whole(Ok(vec![0, 1])),
            "a shrunk list is not the counted list"
        );
        assert!(!whole(Err(i32::MIN)));
    }

    #[test]
    fn a_list_with_no_whole_page_reads_the_exact_range_unproven() {
        let (read, proven) = read_counted_list(
            &mut (),
            500,
            None,
            |_, _| -> Option<Result<Vec<u8>, i32>> { panic!("no whole page was planned") },
            |_| Ok(vec![0; 500]),
        );

        assert_eq!(read.map(|elements| elements.len()), Ok(500));
        assert!(!proven);
    }
}
