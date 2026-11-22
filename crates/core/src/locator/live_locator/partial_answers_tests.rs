use super::PartialAnswers;

#[test]
fn observed_matches_answer_only_an_unreadable_walk_that_found_something() {
    assert!(!PartialAnswers::ObservedMatches.on_unreadable(0));
    assert!(PartialAnswers::ObservedMatches.on_unreadable(1));
    assert!(!PartialAnswers::ObservedMatches.on_limit());
}

#[test]
fn never_and_always_keep_their_contracts() {
    for observed in [0, 3] {
        assert!(!PartialAnswers::Never.on_unreadable(observed));
        assert!(PartialAnswers::Always.on_unreadable(observed));
    }
    assert!(!PartialAnswers::Never.on_limit());
    assert!(PartialAnswers::Always.on_limit());
}
