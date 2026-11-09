//! The global flags `mcp` was started with: checked once, then turned into the arguments every call inherits.

use agent_desktop_core::{
    AppError, ErrorCode,
    response::output::{ErrorPayload, Response},
    session::{resolve_active_session, validate_agent_id},
};
use clap::{Arg, ArgAction, ArgMatches, Command, CommandFactory, parser::ValueSource};
use std::process::ExitCode;

use super::global_scope::GlobalScope;
use crate::cli::Cli;

const SUGGESTION: &str = "Start mcp without them. For visual debug, run snapshot or click with \
--debug --screenshot PATH.html from the CLI; to wait for an element, call desktop_wait.";

/// The server-wide globals given on the command line, as the arguments each child gets. A per-command
/// global, an invalid `--session` or `--agent-id`, or a value that is not UTF-8 is refused with
/// `INVALID_ARGS`, since every call would fail on it; a global without a scope is an internal error.
pub(crate) fn collect_forwarded_globals(matches: &ArgMatches) -> Result<Vec<String>, AppError> {
    let globals = collect_from(&Cli::command(), matches)?;
    if let Ok(Some(session)) = matches.try_get_one::<String>("session") {
        resolve_active_session(Some(session), None)?;
    }
    if let Ok(Some(agent_id)) = matches.try_get_one::<String>("agent_id") {
        validate_agent_id(agent_id)?;
    }
    Ok(globals)
}

/// Turns on the server's own debug logging to stderr when `mcp` was given `--verbose`.
pub(crate) fn init_server_logging(matches: &ArgMatches) {
    if let Ok(Some(true)) = matches.try_get_one::<bool>("verbose") {
        crate::init_tracing(true);
    }
}

/// Writes `error` to stderr as the `mcp` command's JSON envelope, leaving stdout to JSON-RPC, and returns
/// exit code 2 for an argument error, 1 otherwise.
pub(crate) fn refuse_start(error: &AppError) -> ExitCode {
    let response = Response::err("mcp", ErrorPayload::from_app_error(error));
    let _ = crate::cli::emit::write_response(std::io::stderr().lock(), &response, false);
    if error.code() == ErrorCode::InvalidArgs.as_str() {
        ExitCode::from(crate::EXIT_INVALID_ARGS)
    } else {
        ExitCode::FAILURE
    }
}

fn collect_from(root: &Command, matches: &ArgMatches) -> Result<Vec<String>, AppError> {
    let mut globals = Vec::new();
    let mut per_command = Vec::new();
    let mut unclassified = Vec::new();
    for arg in root.get_arguments().filter(|arg| arg.is_global_set()) {
        if matches.value_source(arg.get_id().as_str()) != Some(ValueSource::CommandLine) {
            continue;
        }
        match (GlobalScope::of(arg.get_id().as_str()), arg.get_long()) {
            (Some(GlobalScope::ServerOnly), _) => {}
            (Some(GlobalScope::PerCommand), _) => per_command.push(flag_name(arg)),
            (Some(GlobalScope::ServerWide), Some(long)) => {
                globals.extend(forward(arg, long, matches)?);
            }
            (Some(GlobalScope::ServerWide), None) | (None, _) => unclassified.push(flag_name(arg)),
        }
    }
    if !unclassified.is_empty() {
        return Err(AppError::Internal(format!(
            "mcp: global flag {} is not classified for mcp; give it a forwardable scope in src/mcp/global_scope.rs",
            unclassified.join(", ")
        )));
    }
    if !per_command.is_empty() {
        return Err(AppError::invalid_input_with_suggestion(
            format!(
                "mcp cannot apply {} to every call: each applies to a single command",
                per_command.join(", ")
            ),
            SUGGESTION,
        ));
    }
    Ok(globals)
}

fn forward(arg: &Arg, long: &str, matches: &ArgMatches) -> Result<Vec<String>, AppError> {
    if matches!(arg.get_action(), ArgAction::SetTrue) {
        return Ok(vec![format!("--{long}")]);
    }
    let Some(values) = matches.get_raw(arg.get_id().as_str()) else {
        return Ok(Vec::new());
    };
    values
        .map(|value| {
            value
                .to_str()
                .map(|value| format!("--{long}={value}"))
                .ok_or_else(|| {
                    AppError::invalid_input_with_suggestion(
                        format!("--{long} must be valid UTF-8 for mcp to pass it to every call"),
                        format!("Start mcp with a UTF-8 --{long} value."),
                    )
                })
        })
        .collect()
}

fn flag_name(arg: &Arg) -> String {
    match (arg.get_long(), arg.get_short()) {
        (Some(long), _) => format!("--{long}"),
        (None, Some(short)) => format!("-{short}"),
        (None, None) => arg.get_id().to_string(),
    }
}

#[cfg(test)]
#[path = "global_forward_tests.rs"]
mod tests;
