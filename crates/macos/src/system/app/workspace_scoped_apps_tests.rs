use super::*;

fn app(name: &str, pid: u32) -> AppInfo {
    AppInfo {
        name: name.to_string(),
        pid: agent_desktop_core::ProcessId::new(pid),
        bundle_id: None,
        process_instance: Some(format!("instance-{pid}")),
        presentation: None,
    }
}

fn inventory(apps: Vec<AppInfo>) -> AppInventory {
    AppInventory {
        apps,
        skipped: Vec::new(),
    }
}

fn names(inventory: &AppInventory) -> Vec<&str> {
    inventory.apps.iter().map(|app| app.name.as_str()).collect()
}

#[test]
fn a_named_match_is_answered_without_reading_every_app() {
    let mut reads = Vec::new();
    let result = scoped_inventory("Calculator", |named_only| {
        reads.push(named_only);
        Ok((inventory(vec![app("Calculator", 7)]), false))
    })
    .unwrap();

    assert_eq!(reads, vec![true]);
    assert_eq!(names(&result), vec!["Calculator"]);
}

#[test]
fn no_surviving_named_match_reads_every_app_so_an_alias_still_matches() {
    let mut reads = Vec::new();
    let result = scoped_inventory("calculator", |named_only| {
        reads.push(named_only);
        Ok((
            inventory(if named_only {
                Vec::new()
            } else {
                vec![app("Calculator Helper", 9)]
            }),
            !named_only,
        ))
    })
    .unwrap();

    assert_eq!(reads, vec![true, false]);
    assert_eq!(names(&result), vec!["Calculator Helper"]);
}

#[test]
fn a_full_read_keeps_only_display_name_matches_when_one_exists() {
    let result = scoped_inventory("Preview", |named_only| {
        Ok((
            inventory(if named_only {
                vec![app("Other", 3)]
            } else {
                vec![app("Preview", 4), app("Preview Helper", 5)]
            }),
            !named_only,
        ))
    })
    .unwrap();

    assert_eq!(names(&result), vec!["Preview"]);
}

#[test]
fn a_read_that_already_holds_every_app_is_not_repeated() {
    let mut reads = Vec::new();
    let result = scoped_inventory("calculator", |named_only| {
        reads.push(named_only);
        Ok((inventory(vec![app("Calculator Helper", 9)]), true))
    })
    .unwrap();

    assert_eq!(reads, vec![true]);
    assert_eq!(names(&result), vec!["Calculator Helper"]);
}

#[test]
fn a_failed_named_read_fails_the_lookup() {
    let mut reads = 0;
    let error = scoped_inventory("Preview", |_| {
        reads += 1;
        Err(AdapterError::timeout("inventory timed out"))
    })
    .unwrap_err();

    assert_eq!(reads, 1);
    assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
}

#[test]
fn a_name_no_app_has_reads_every_record_in_the_same_read() {
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded().unwrap();
    let bytes = crate::system::appkit_bridge::named_workspace_snapshot_json(&|_| false).unwrap();
    let all = record_names(&crate::system::appkit_bridge::workspace_snapshot_json().unwrap());

    assert!(!selected_only(&bytes));
    assert_eq!(record_names(&bytes).len(), all.len());
}

fn record_names(bytes: &[u8]) -> Vec<String> {
    let snapshot: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    snapshot["applications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn the_named_workspace_read_returns_records_only_for_selected_names() {
    crate::system::cocoa_runtime::ensure_cocoa_multithreaded().unwrap();
    let all = record_names(&crate::system::appkit_bridge::workspace_snapshot_json().unwrap());
    let Some(target) = all.first().cloned() else {
        return;
    };

    let named = record_names(
        &crate::system::appkit_bridge::named_workspace_snapshot_json(&|name| name == target)
            .unwrap(),
    );

    assert!(!named.is_empty());
    assert!(named.iter().all(|name| *name == target));
    assert!(selected_only(
        &crate::system::appkit_bridge::named_workspace_snapshot_json(&|name| name == target)
            .unwrap()
    ));
    assert_eq!(
        named.len(),
        all.iter().filter(|name| **name == target).count()
    );
}
