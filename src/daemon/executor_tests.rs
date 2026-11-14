use super::exit_byte;
use std::process::ExitCode;

#[test]
fn exit_statuses_map_to_their_bytes() {
    assert_eq!(exit_byte(ExitCode::SUCCESS), 0);
    assert_eq!(exit_byte(ExitCode::FAILURE), 1);
    assert_eq!(exit_byte(ExitCode::from(2)), 2);
}
