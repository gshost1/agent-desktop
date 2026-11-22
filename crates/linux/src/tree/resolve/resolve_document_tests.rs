use super::resolve;
use super::resolve_fake::{FRAME_PATH, Fake, deadline, node};
use crate::tree::Field;

#[test]
fn paragraph_ref_resolves_after_typing_changes_its_value() {
    for role in [73, 83, 94] {
        let mut fake = Fake::new();
        let mut paragraph = node(role, "", 600, 24);
        paragraph.value.value = Field::Present("before typing".into());
        fake.add(FRAME_PATH, "/paragraph", paragraph);
        let mut entry = fake.entry_of("/paragraph", true);
        entry.identity.value = Some("before typing".into());
        assert_eq!(
            resolve(&mut fake, &entry, deadline()).unwrap().path(),
            "/paragraph"
        );
        fake.nodes.get_mut("/paragraph").unwrap().read.value.value =
            Field::Present("after typing".into());
        assert_eq!(
            resolve(&mut fake, &entry, deadline()).unwrap().path(),
            "/paragraph"
        );
    }
}
