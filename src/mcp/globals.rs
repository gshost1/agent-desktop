//! The global flags `mcp` was started with, split into the ones every call inherits and the ones it refuses.

use agent_desktop_core::{
    AppError,
    response::output::{ErrorPayload, Response},
};
use clap::{ArgAction, ArgMatches, CommandFactory, parser::ValueSource};
use std::process::ExitCode;
use std::sync::Mutex;

use super::global_scope::GlobalScope;
use crate::cli::Cli;

const SUGGESTION: &str = "Start mcp without them. For visual debug, run snapshot or click with \
--debug --screenshot PATH.html from the CLI; to wait for an element, call desktop_wait.";

/// The server-wide globals given on the command line, as the arguments each child gets. Any per-command or
/// unknown global given there is refused with `INVALID_ARGS`, since the server would pass it to every call.
pub(crate) fn forwarded_globals(matches: &ArgMatches) -> Result<Vec<String>, AppError> {
    let root = Cli::command();
    let mut globals = Vec::new();
    let mut refused = Vec::new();
    for arg in root.get_arguments().filter(|arg| arg.is_global_set()) {
        let id = arg.get_id().as_str();
        let Some(long) = arg.get_long() else { continue };
        if matches.value_source(id) != Some(ValueSource::CommandLine) {
            continue;
        }
        if GlobalScope::of(id) != Some(GlobalScope::ServerWide) {
            refused.push(format!("--{long}"));
        } else if matches!(arg.get_action(), ArgAction::SetTrue) {
            globals.push(format!("--{long}"));
        } else if let Some(values) = matches.get_raw(id) {
            globals.extend(values.map(|value| format!("--{long}={}", value.to_string_lossy())));
        }
    }
    if refused.is_empty() {
        return Ok(globals);
    }
    Err(AppError::invalid_input_with_suggestion(
        format!(
            "mcp cannot apply {} to every call: each applies to a single command",
            refused.join(", ")
        ),
        SUGGESTION,
    ))
}

/// Prints `error` as the `mcp` command's JSON envelope and returns the structured-error exit code.
pub(crate) fn refuse_start(error: &AppError) -> ExitCode {
    let response = Response::err("mcp", ErrorPayload::from_app_error(error));
    match serde_json::to_value(&response) {
        Ok(envelope) => super::write_line(&Mutex::new(std::io::stdout()), &envelope),
        Err(error) => eprintln!("agent-desktop mcp: {error}"),
    }
    ExitCode::FAILURE
}

#[cfg(test)]
#[path = "globals_tests.rs"]
mod tests;
