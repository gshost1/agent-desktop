//! A scoped lookup driven by scripted AppKit payloads, through the closure
//! that turns the named read's `selected_only` flag into the decision to
//! read every app.

use super::*;
use std::cell::RefCell;

const ALIAS: &str = "com.example.scoped-lookup";

fn deadline() -> Instant {
    Instant::now() + std::time::Duration::from_secs(5)
}

fn record(name: &str, bundle_id: &str, policy: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "pid": std::process::id(),
        "bundle_id": bundle_id,
        "launch_time": 1.0,
        "activation_policy": policy
    })
}

fn payload(records: Vec<serde_json::Value>, selected_only: bool) -> Vec<u8> {
    let mut snapshot = serde_json::json!({ "applications": records, "frontmost_pid": 0, "frontmost_launch_time": null });
    if selected_only {
        snapshot["selected_only"] = serde_json::Value::Bool(true);
    }
    serde_json::to_vec(&snapshot).unwrap()
}

struct Script {
    named: Vec<u8>,
    every: Vec<u8>,
    reads: RefCell<Vec<&'static str>>,
}

impl Script {
    fn look_up(&self, name: &str) -> Result<AppInventory, AdapterError> {
        crate::system::cocoa_runtime::ensure_cocoa_multithreaded().unwrap();
        let reads = WorkspaceReads {
            named: |selects: &dyn Fn(&str) -> bool| {
                assert!(selects(name));
                self.reads.borrow_mut().push("named");
                Ok(self.named.clone())
            },
            every: || {
                self.reads.borrow_mut().push("every");
                Ok(self.every.clone())
            },
        };
        list_apps_scoped_from(&reads, name, None, deadline())
    }
}

#[test]
fn a_selected_read_whose_record_the_filter_drops_reads_every_app_for_an_alias() {
    let script = Script {
        named: payload(vec![record(ALIAS, "com.example.other", "prohibited")], true),
        every: payload(vec![record("Scoped Lookup", ALIAS, "regular")], false),
        reads: RefCell::new(Vec::new()),
    };

    let inventory = script.look_up(ALIAS).unwrap();

    assert_eq!(*script.reads.borrow(), vec!["named", "every"]);
    let names: Vec<_> = inventory.apps.iter().map(|app| app.name.as_str()).collect();
    assert_eq!(names, vec!["Scoped Lookup"]);
    assert_eq!(inventory.apps[0].bundle_id.as_deref(), Some(ALIAS));
}

#[test]
fn a_read_that_held_every_record_is_not_followed_by_a_second_read() {
    let script = Script {
        named: payload(
            vec![record(ALIAS, "com.example.other", "prohibited")],
            false,
        ),
        every: payload(vec![record("Scoped Lookup", ALIAS, "regular")], false),
        reads: RefCell::new(Vec::new()),
    };

    let inventory = script.look_up(ALIAS).unwrap();

    assert_eq!(*script.reads.borrow(), vec!["named"]);
    assert!(inventory.apps.is_empty());
}
