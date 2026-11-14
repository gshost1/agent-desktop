use super::*;

const BASELINE_ID: &str = "sbaseline";
const SNAPSHOT_ID: &str = "snewsnap";

fn normalized(data: &Value, since: &str) -> String {
    let now = snapshot_id(data);
    serde_json::to_string_pretty(data)
        .unwrap()
        .replace(&now, SNAPSHOT_ID)
        .replace(since, BASELINE_ID)
}

fn changed_window_diff() -> (Value, String) {
    let adapter = ChangingWindow::new(before());
    let context = CommandContext::default();
    let first = crate::commands::snapshot::execute(args(), &adapter, &context).unwrap();
    let since = snapshot_id(&first);
    adapter.show(after());
    (
        execute_since(args(), &since, &adapter, &context).unwrap(),
        since,
    )
}

#[test]
fn a_diff_matches_its_golden_json() {
    let _home = HomeGuard::new();
    let (data, since) = changed_window_diff();
    let golden = include_str!("../../../../tests/fixtures/snapshot-since-diff.json");
    assert_eq!(normalized(&data, &since), golden.trim_end());
}

#[test]
fn a_diff_renders_to_its_golden_lines() {
    let _home = HomeGuard::new();
    let (mut data, since) = changed_window_diff();
    assert!(crate::snapshot::diff::diff_lines::replace_diff_with_lines(&mut data).unwrap());
    let lines: Vec<&str> = data["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let golden = include_str!("../../../../tests/fixtures/snapshot-since-diff.lines.txt");
    assert_eq!(
        lines
            .join("\n")
            .replace(&snapshot_id(&data), SNAPSHOT_ID)
            .replace(&since, BASELINE_ID),
        golden.trim_end()
    );
}
