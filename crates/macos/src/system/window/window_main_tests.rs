use super::decide_main;

#[test]
fn both_reports_naming_one_candidate_choose_it() {
    assert_eq!(
        decide_main(2, &[(2, Some(true)), (1, Some(false))], &[1, 2]),
        Some(2)
    );
}

#[test]
fn an_application_naming_another_window_is_a_conflict() {
    assert_eq!(
        decide_main(1, &[(2, Some(true)), (1, Some(false))], &[1, 2]),
        None
    );
}

#[test]
fn two_windows_that_both_say_main_are_no_choice() {
    assert_eq!(
        decide_main(2, &[(2, Some(true)), (1, Some(true))], &[1, 2]),
        None
    );
}

#[test]
fn no_window_that_says_main_is_no_choice() {
    assert_eq!(
        decide_main(2, &[(2, Some(false)), (1, Some(false))], &[1, 2]),
        None
    );
}

#[test]
fn a_candidate_whose_main_state_was_not_read_is_no_proof() {
    assert_eq!(decide_main(2, &[(2, Some(true)), (1, None)], &[1, 2]), None);
    assert_eq!(decide_main(2, &[(2, Some(true))], &[1, 2]), None);
}

#[test]
fn a_main_window_outside_the_candidates_is_no_choice() {
    assert_eq!(
        decide_main(
            3,
            &[(3, Some(true)), (1, Some(false)), (2, Some(false))],
            &[1, 2]
        ),
        None
    );
}

#[test]
fn a_window_set_aside_as_auxiliary_does_not_block_the_choice() {
    assert_eq!(
        decide_main(
            2,
            &[(5, Some(false)), (2, Some(true)), (1, Some(false))],
            &[1, 2]
        ),
        Some(2)
    );
}
