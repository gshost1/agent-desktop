use super::*;

fn other_thread() -> ThreadId {
    std::thread::spawn(|| std::thread::current().id())
        .join()
        .unwrap()
}

#[test]
fn a_free_lease_is_taken_by_its_caller() {
    let caller = std::thread::current().id();
    let mut state = ProcessLeaseState::Free;
    assert_eq!(state.acquire(caller), Ok(()));
    assert_eq!(state, ProcessLeaseState::Held { owner: caller });
}

#[test]
fn a_held_lease_reports_its_owner_and_stays_held() {
    let owner = other_thread();
    let caller = std::thread::current().id();
    let mut state = ProcessLeaseState::Held { owner };
    assert_eq!(state.acquire(caller), Err(owner));
    assert_eq!(state.acquire(owner), Err(owner));
    assert_eq!(state, ProcessLeaseState::Held { owner });
}

#[test]
fn release_frees_the_lease_for_any_thread() {
    let mut state = ProcessLeaseState::Held {
        owner: other_thread(),
    };
    state.release();
    assert_eq!(state, ProcessLeaseState::Free);
    assert_eq!(state.acquire(std::thread::current().id()), Ok(()));
}
