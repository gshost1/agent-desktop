//! Which invocations the daemon takes. It declines, before anything runs, every invocation that
//! depends on the calling process itself: stdin input, `launch` (the app inherits the caller's
//! whole environment), `mcp`, `--verbose` logging, help output and argument errors.

use clap::FromArgMatches;

use crate::cli::{Cli, Commands};
use crate::command_policy::CommandEffect;

/// An invocation the daemon runs, with what a client needs if the daemon dies mid-run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Admitted {
    pub(crate) command: &'static str,
    pub(crate) rerun_safe: bool,
}

/// Admits `argv` (program name first) or names why the caller must run it itself.
pub(crate) fn admit(argv: &[String]) -> Result<Admitted, &'static str> {
    let matches = crate::root_command()
        .try_get_matches_from(argv)
        .map_err(|_| "arguments")?;
    if matches.subcommand_name() == Some("mcp") {
        return Err("mcp");
    }
    let mut cli = Cli::from_arg_matches(&matches).map_err(|_| "arguments")?;
    if cli.visual_debug.verbose {
        return Err("verbose");
    }
    let mut command = cli.command.take().ok_or("help")?;
    if crate::cli_args::stdin::input(&mut command).is_some() {
        return Err("stdin");
    }
    if launches_an_app(&command) {
        return Err("launch");
    }
    Ok(Admitted {
        command: command.name(),
        rerun_safe: command.effect() == CommandEffect::ReadOnly,
    })
}

fn launches_an_app(command: &Commands) -> bool {
    match command {
        Commands::Launch(_) => true,
        Commands::Batch(args) => batch_names_launch(&args.commands_json),
        _ => false,
    }
}

/// A batch whose JSON cannot be read is treated as one that launches: the in-process run
/// reports the parse error.
fn batch_names_launch(json: &str) -> bool {
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str::<serde_json::Value>(json)
    else {
        return true;
    };
    entries.iter().any(|entry| {
        entry
            .get("command")
            .and_then(serde_json::Value::as_str)
            .is_none_or(|name| name == "launch")
    })
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
