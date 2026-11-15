use super::*;

#[test]
fn bytes_up_to_the_limit_pass_and_one_more_is_refused() {
    let mut writer = CappedWriter::new(Vec::new(), 4);
    writer.write_all(b"abcd").unwrap();
    let err = writer.write_all(b"e").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::FileTooLarge);
    assert_eq!(writer.into_inner(), b"abcd");
}
