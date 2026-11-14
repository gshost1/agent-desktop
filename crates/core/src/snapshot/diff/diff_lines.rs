//! The `--lines` reading form of a `snapshot --since` diff: one line per row,
//! marked `+` added, `~` changed and `-` removed, then the focused element.

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{AccessibilityNode, AppError, snapshot::ref_lines};

const ROW_KEYS: [(&str, &str); 3] = [("added", "+ "), ("changed", "~ "), ("removed", "- ")];

/// Replaces the rows of `data.diff` with `data.lines` and leaves `since`,
/// `status` and every other key as they were. Returns `false`, changing
/// nothing, when `data` holds no comparable diff or already holds lines; a
/// row that does not read back leaves `data` unchanged and is an error.
pub fn replace_diff_with_lines(data: &mut Value) -> Result<bool, AppError> {
    let Some(body) = data.as_object_mut() else {
        return Ok(false);
    };
    if body.contains_key("lines") {
        return Ok(false);
    }
    let Some(diff) = body.get("diff").and_then(Value::as_object) else {
        return Ok(false);
    };
    if diff.get("status").and_then(Value::as_str) == Some("not_comparable") {
        return Ok(false);
    }
    let lines = diff_lines(diff)?;
    if let Some(diff) = body.get_mut("diff").and_then(Value::as_object_mut) {
        for (key, _) in ROW_KEYS {
            diff.remove(key);
        }
        diff.remove("focused");
    }
    body.insert("lines".into(), Value::from(lines));
    Ok(true)
}

fn diff_lines(diff: &Map<String, Value>) -> Result<Vec<String>, AppError> {
    let mut lines = Vec::new();
    if diff.get("status").and_then(Value::as_str) == Some("unchanged") {
        lines.push("no change".to_owned());
    }
    for (key, marker) in ROW_KEYS {
        for row in diff.get(key).and_then(Value::as_array).into_iter().flatten() {
            let mut line = format!("{marker}{}", row_line(row)?);
            if key == "changed" {
                push_was(&mut line, row);
            }
            lines.push(line);
        }
    }
    if let Some(focused) = diff.get("focused") {
        lines.push(format!("focused {}", row_line(focused)?));
    }
    Ok(lines)
}

fn row_line(row: &Value) -> Result<String, AppError> {
    let node = AccessibilityNode::deserialize(row)?;
    Ok(ref_lines::render_ref_lines(&node)
        .into_iter()
        .next()
        .unwrap_or_default())
}

fn push_was(line: &mut String, row: &Value) {
    let Some(was) = row.get("was").and_then(Value::as_object) else {
        return;
    };
    let mut keys: Vec<&str> = row
        .get("changed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|field| if field == "focus" { "states" } else { field })
        .collect();
    keys.dedup();
    let parts: Vec<String> = keys
        .into_iter()
        .filter_map(|key| was.get(key).map(|value| was_part(key, value)))
        .collect();
    if !parts.is_empty() {
        line.push_str(" (was ");
        line.push_str(&parts.join(", "));
        line.push(')');
    }
}

fn was_part(key: &str, value: &Value) -> String {
    let label = if key == "description" { "desc" } else { key };
    match value {
        Value::String(text) => format!("{label}={}", ref_lines::quote_text(text)),
        Value::Array(states) => {
            let states: Vec<&str> = states.iter().filter_map(Value::as_str).collect();
            format!("[{}]", states.join(", "))
        }
        _ => format!("no {label}"),
    }
}

#[cfg(test)]
#[path = "diff_lines_tests.rs"]
mod tests;
