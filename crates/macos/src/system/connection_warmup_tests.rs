use super::*;

fn settle() {
    let deadline = Instant::now() + Duration::from_secs(5);
    while in_flight() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn building_the_adapter_starts_the_window_server_connection() {
    let _adapter = crate::MacOSAdapter::new();
    assert!(window_server_started());
    settle();
    assert_eq!(in_flight(), 0);
}

#[test]
fn each_application_is_warmed_once_and_only_a_few_are_warmed() {
    let own = std::process::id() as i32;
    start_applications([own, own, 0, -1]);
    start_applications([own]);
    let warmed = warmed_applications();
    assert_eq!(warmed.iter().filter(|pid| **pid == own).count(), 1);
    assert!(!warmed.contains(&0) && !warmed.contains(&-1));
    start_applications(i32::MAX - 8..i32::MAX);
    assert!(warmed_applications().len() <= MAX_WARMED_APPLICATIONS);
    settle();
    assert_eq!(in_flight(), 0);
}
