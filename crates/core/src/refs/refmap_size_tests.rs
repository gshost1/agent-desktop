use super::super::tests::entry;
use super::*;

fn map_of(names: &[&str]) -> RefMap {
    let mut map = RefMap::new();
    for name in names {
        map.allocate(entry("button", Some(name)));
    }
    map
}

#[test]
fn entry_lengths_and_commas_add_up_to_the_stored_length() {
    let map = map_of(&["OK", "Cancel", "Save \"quoted\" \u{e9}", "Open"]);
    let stored = serde_json::to_string(&map).unwrap().len();

    let entries: usize = (1..=4)
        .map(|index| {
            map.serialized_entry_len(&format!("@e{index}"))
                .unwrap()
                .unwrap()
        })
        .sum();
    let commas = map.len() - 1;

    assert_eq!(map.serialized_len().unwrap(), stored);
    assert_eq!(
        map.serialized_len_without_entries().unwrap() + entries + commas,
        stored
    );
}

#[test]
fn an_absent_ref_has_no_entry_length() {
    let map = map_of(&["OK"]);
    assert_eq!(map.serialized_entry_len("@e9").unwrap(), None);
}

#[test]
fn retaining_refs_keeps_ids_and_the_counter() {
    let mut map = map_of(&["A", "B", "C"]);
    map.retain_refs(&HashSet::from(["@e1".to_string(), "@e3".to_string()]));

    assert_eq!(map.len(), 2);
    assert!(map.get("@e2").is_none());
    assert_eq!(map.get("@e3").unwrap().identity.name.as_deref(), Some("C"));
    map.validate().unwrap();
    assert_eq!(map.allocate(entry("button", Some("D"))), "@e4");
}
