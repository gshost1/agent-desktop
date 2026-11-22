use super::reap_exited_children;

#[test]
fn exited_children_are_collected_without_blocking() {
    let probe = unsafe { libc::fork() };
    assert!(probe >= 0);
    if probe == 0 {
        for _ in 0..2 {
            if unsafe { libc::fork() } == 0 {
                unsafe { libc::_exit(0) };
            }
        }
        unsafe { libc::usleep(200_000) };
        let first = reap_exited_children();
        let second = reap_exited_children();
        let code = if first == 2 && second == 0 { 0 } else { 1 };
        unsafe { libc::_exit(code) };
    }
    let mut status = 0;
    assert_eq!(unsafe { libc::waitpid(probe, &raw mut status, 0) }, probe);

    assert!(libc::WIFEXITED(status));
    assert_eq!(libc::WEXITSTATUS(status), 0);
}
