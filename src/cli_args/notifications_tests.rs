use super::*;

#[test]
fn dismiss_without_index_or_key_is_a_parse_error() {
    let error = DismissNotificationCliArgs::try_parse_from(["dismiss-notification"])
        .expect_err("neither INDEX nor --key names an entry");

    assert_eq!(
        error.kind(),
        clap::error::ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn dismiss_takes_a_key_alone_or_an_index_alone() {
    let keyed = DismissNotificationCliArgs::try_parse_from(["dismiss-notification", "--key", "k"])
        .expect("--key alone parses");
    let indexed = DismissNotificationCliArgs::try_parse_from(["dismiss-notification", "2"])
        .expect("INDEX alone parses");

    assert_eq!(keyed.key.as_deref(), Some("k"));
    assert_eq!(indexed.index, Some(2));
}
