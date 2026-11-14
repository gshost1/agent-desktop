//! Writes a command's response envelope to stdout, applying `--lines` first.

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use agent_desktop_core::{
    AppError,
    response::output::{ErrorPayload, Response},
    snapshot::{diff::diff_lines::replace_diff_with_lines, ref_lines::replace_tree_with_lines},
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

/// Replaces the snapshot tree or `snapshot --since` diff rows in `data`, and
/// in each completed entry of a batch, with their lines. A tree that cannot be rendered stays a tree and
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
    match replace_tree_with_lines(data)
        .and_then(|tree| Ok(replace_diff_with_lines(data)? || tree))
    {
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
