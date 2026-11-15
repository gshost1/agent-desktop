use serde_json::json;

use super::with_entry;

#[test]
fn details_that_are_not_an_object_are_kept_beside_the_restore() {
    let details = with_entry(Some(json!(["kept"])), "list_restore", "unverified");

    assert_eq!(details["error_details"], json!(["kept"]));
    assert_eq!(details["list_restore"], "unverified");
}
