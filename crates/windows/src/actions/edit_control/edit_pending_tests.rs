use super::PendingReplacements;

#[test]
fn a_pool_hands_out_no_more_slots_than_its_capacity() {
    static POOL: PendingReplacements = PendingReplacements::new(2);
    let first = POOL.try_acquire().expect("first slot");
    let second = POOL.try_acquire().expect("second slot");
    assert_eq!(POOL.in_flight(), 2);
    assert!(POOL.try_acquire().is_none(), "a third slot is refused");
    assert_eq!(POOL.in_flight(), 2, "a refusal holds nothing");
    drop(first);
    assert_eq!(POOL.in_flight(), 1);
    let third = POOL.try_acquire().expect("a released slot is reused");
    assert!(POOL.try_acquire().is_none());
    drop(second);
    drop(third);
    assert_eq!(POOL.in_flight(), 0);
}

#[test]
fn a_pool_without_capacity_refuses_every_slot() {
    static POOL: PendingReplacements = PendingReplacements::new(0);
    assert!(POOL.try_acquire().is_none());
    assert_eq!(POOL.in_flight(), 0);
}

#[test]
fn slots_taken_from_many_threads_never_pass_the_capacity() {
    static POOL: PendingReplacements = PendingReplacements::new(3);
    let permits: Vec<_> = (0..16)
        .map(|_| std::thread::spawn(|| POOL.try_acquire()))
        .map(|worker| worker.join().expect("worker joins"))
        .collect();
    assert_eq!(permits.iter().filter(|permit| permit.is_some()).count(), 3);
    assert_eq!(POOL.in_flight(), 3);
    drop(permits);
    assert_eq!(POOL.in_flight(), 0);
}
