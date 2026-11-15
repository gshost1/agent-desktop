//! The one-line error summary written to stderr beside an error envelope. A
//! caller that pipes stdout into `jq`, `grep` or `head` loses the envelope's
//! error to the filter; the stderr line keeps the code, message and suggestion
//! in sight. Stdout and the exit code never change.

use std::io::Write;

use agent_desktop_core::response::output::Response;
use serde_json::Value;

/// Set to `0` or `off` to keep stderr free of the summary lines.
pub(crate) const ERROR_LINE_ENV: &str = "AGENT_DESKTOP_STDERR_SUMMARY";
const MAX_FIELD_CHARS: usize = 400;

/// Writes the summary lines for `response` to stderr, unless the caller turned
/// them off with [`ERROR_LINE_ENV`]. Every path that prints an envelope calls
/// this one function, so each prints the same line.
pub(crate) fn report_response_errors(response: &Response) {
    if error_lines_enabled(std::env::var(ERROR_LINE_ENV).ok().as_deref()) {
        write_lines(&response_error_lines(response));
    }
}

/// One line for an error envelope, one per failed completed entry of a
/// successful `batch`, none otherwise.
pub(crate) fn response_error_lines(response: &Response) -> Vec<String> {
    match (&response.error, &response.data) {
        (Some(error), _) => vec![summary_line(
            "",
            &error.code,
            &error.message,
            error.suggestion.as_deref(),
        )],
        (None, Some(data)) if response.ok && response.command == "batch" => batch_entry_lines(data),
        _ => Vec::new(),
    }
}

pub(crate) fn error_lines_enabled(setting: Option<&str>) -> bool {
    !matches!(setting.map(str::trim), Some("0" | "off" | "false"))
}

fn batch_entry_lines(data: &Value) -> Vec<String> {
    let Some(entries) = data["results"].as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter(|entry| entry["ok"] == Value::Bool(false) && entry["execution"] == "completed")
        .map(|entry| {
            let index = entry["index"]
                .as_u64()
                .map_or_else(String::new, |index| index.to_string());
            let command = one_line(entry["command"].as_str().unwrap_or(""));
            error_object_line(&format!("batch[{index}] {command}: "), &entry["error"])
        })
        .collect()
}

fn error_object_line(prefix: &str, error: &Value) -> String {
    summary_line(
        prefix,
        error["code"].as_str().unwrap_or("ERROR"),
        error["message"].as_str().unwrap_or(""),
        error["suggestion"].as_str(),
    )
}

fn summary_line(prefix: &str, code: &str, message: &str, suggestion: Option<&str>) -> String {
    let mut line = format!(
        "agent-desktop: {prefix}{}: {}",
        one_line(code),
        one_line(message)
    );
    if let Some(suggestion) = suggestion.filter(|text| !text.trim().is_empty()) {
        line.push_str(" -- ");
        line.push_str(&one_line(suggestion));
    }
    line
}

fn one_line(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    crate::diagnostic::bounded_text(flat.trim(), MAX_FIELD_CHARS)
}

fn write_lines(lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    for line in lines {
        let _ = writeln!(stderr, "{line}");
    }
    let _ = stderr.flush();
}

#[cfg(test)]
#[path = "error_line_tests.rs"]
mod tests;
