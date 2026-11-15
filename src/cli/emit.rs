//! Writes a command's response envelope to stdout, applying `--lines` first.

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use agent_desktop_core::{
    AppError,
    response::output::{ErrorPayload, Response},
    snapshot::ref_lines::replace_tree_with_lines,
};
use serde_json::Value;

/// Prints the envelope for `result` and returns the exit code. With `lines`,
/// every snapshot tree in a successful result becomes `data.lines` and the
/// envelope is indented so each line sits on its own output line.
pub(crate) fn finish(cmd_name: &str, result: Result<Value, AppError>, lines: bool) -> ExitCode {
    let (result, pretty) = present_lines(cmd_name, result, lines);
    let response = response(cmd_name, result);
    if let Err(write_err) = emit_response(&response, pretty) {
        return report_output_failure(write_err);
    }
    report_failure_notice(&response);
    if response.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

pub(crate) fn report_output_failure(write_err: std::io::Error) -> ExitCode {
    eprintln!("agent-desktop: failed to write response to stdout: {write_err}");
    ExitCode::FAILURE
}

/// Prints a failed response's error as one stderr line, so a pipe that
/// filters or cuts stdout cannot hide that the command failed.
pub(crate) fn report_failure_notice(response: &Response) {
    if let Some(notice) = failure_notice(response) {
        let _ = writeln!(std::io::stderr().lock(), "{notice}");
    }
}

/// `agent-desktop: <CODE>: <message> -- <suggestion>` for a failed response,
/// on one line and bounded in length.
pub(crate) fn failure_notice(response: &Response) -> Option<String> {
    let error = response.error.as_ref().filter(|_| !response.ok)?;
    let mut notice = format!("agent-desktop: {}: {}", error.code, error.message);
    if let Some(suggestion) = error.suggestion.as_deref() {
        notice.push_str(" -- ");
        notice.push_str(suggestion);
    }
    let one_line = notice.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(crate::diagnostic::bounded_text(&one_line, NOTICE_MAX_CHARS))
}

const NOTICE_MAX_CHARS: usize = 1024;

pub(crate) fn emit_response(response: &impl serde::Serialize, pretty: bool) -> std::io::Result<()> {
    write_response(std::io::stdout().lock(), response, pretty)
}

/// Writes `response` to `out` as one JSON line, or indented when `pretty`,
/// and flushes it.
pub(crate) fn write_response(
    out: impl Write,
    response: &impl serde::Serialize,
    pretty: bool,
) -> std::io::Result<()> {
    let mut writer = BufWriter::new(out);
    if pretty {
        serde_json::to_writer_pretty(&mut writer, response).map_err(std::io::Error::other)?;
    } else {
        serde_json::to_writer(&mut writer, response).map_err(std::io::Error::other)?;
    }
    writer.write_all(b"\n")?;
    writer.flush()
}

fn response(cmd_name: &str, result: Result<Value, AppError>) -> Response {
    match result {
        Ok(data) => Response::ok(cmd_name, data),
        Err(error) => Response::err(cmd_name, ErrorPayload::from_app_error(&error)),
    }
}

fn present_lines(
    cmd_name: &str,
    result: Result<Value, AppError>,
    lines: bool,
) -> (Result<Value, AppError>, bool) {
    match result {
        Ok(mut data) if lines => {
            let replaced = apply_lines_output(cmd_name, &mut data);
            (Ok(data), replaced)
        }
        other => (other, false),
    }
}

/// Replaces the snapshot tree in `data`, and in each completed entry of a
/// batch, with its lines. A tree that cannot be rendered stays a tree and
/// gains `lines_error`, so a delivered action's report is never turned into
/// an error and the caller can tell why `lines` is missing.
pub(crate) fn apply_lines_output(cmd_name: &str, data: &mut Value) -> bool {
    let mut replaced = replace_or_mark(data);
    if cmd_name == "batch"
        && let Some(entries) = data.get_mut("results").and_then(Value::as_array_mut)
    {
        for entry in entries.iter_mut().filter_map(|entry| entry.get_mut("data")) {
            replaced |= replace_or_mark(entry);
        }
    }
    replaced
}

fn replace_or_mark(data: &mut Value) -> bool {
    match replace_tree_with_lines(data) {
        Ok(replaced) => replaced,
        Err(error) => {
            if let Some(body) = data.as_object_mut() {
                body.insert("lines_error".into(), Value::from(error.to_string()));
            }
            false
        }
    }
}

#[cfg(test)]
#[path = "emit_tests.rs"]
mod tests;
