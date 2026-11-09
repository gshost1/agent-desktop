use std::path::PathBuf;

use clap::Args;

#[derive(Args, Debug, Default)]
pub(crate) struct OutputArgs {
    #[arg(
        long,
        global = true,
        help = "Append reliability trace JSONL to this path"
    )]
    pub trace: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Fail on trace setup/pre-action write errors"
    )]
    pub trace_strict: bool,
    #[arg(
        long,
        global = true,
        help = "Print snapshot trees as data.lines, one indented line per node with its ref first (snapshot and --wait-for results)"
    )]
    pub lines: bool,
}
