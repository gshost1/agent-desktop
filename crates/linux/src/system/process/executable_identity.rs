use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

/// Executable evidence shared by launch attachment and desktop aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableIdentity {
    pub(crate) executable: PathBuf,
    pub(crate) argv: Option<Vec<OsString>>,
}

impl ExecutableIdentity {
    pub(crate) fn requested(argv: &[String], path: &[PathBuf]) -> Option<Self> {
        let program = argv.first()?;
        let file = if program.contains('/') {
            let file = PathBuf::from(program);
            if !file.is_absolute() {
                return None;
            }
            file
        } else {
            crate::system::launch::launch_target::find_on_path(program, path)?
        };
        let executable = file.canonicalize().ok()?;
        let argv = Some({
            std::iter::once(file.into_os_string())
                .chain(argv[1..].iter().map(OsString::from))
                .collect()
        });
        Some(Self { executable, argv })
    }

    pub(crate) fn read(pid: u32) -> Option<Self> {
        let executable = std::fs::canonicalize(format!("/proc/{pid}/exe")).ok()?;
        let argv = std::fs::read(format!("/proc/{pid}/cmdline"))
            .ok()
            .and_then(|bytes| {
                let words: Vec<OsString> = bytes
                    .strip_suffix(&[0])?
                    .split(|b| *b == 0)
                    .map(|word| OsString::from_vec(word.to_vec()))
                    .collect();
                Some(words)
            });
        Some(Self { executable, argv })
    }

    /// Whether a command written as `words` (a desktop entry's expanded
    /// Exec, before PATH resolution) could match this running process, from
    /// the words alone: the program's name is the invocation's name and the
    /// arguments are equal. Every command [`Self::matches`] accepts for this
    /// process passes, so a caller can skip resolving the ones that fail.
    pub(crate) fn could_be_run_by(&self, words: &[String]) -> bool {
        let (Some((program, arguments)), Some((invocation, actual_arguments))) = (
            words.split_first(),
            self.argv.as_deref().and_then(<[_]>::split_first),
        ) else {
            return false;
        };
        Path::new(program).file_name() == Path::new(invocation).file_name()
            && arguments_equal(arguments.iter().map(OsStr::new), actual_arguments)
    }

    pub(crate) fn matches(&self, running: &Self) -> bool {
        self.executable == running.executable
            && self.argv.as_ref().is_some_and(|argv| {
                running
                    .argv
                    .as_deref()
                    .is_some_and(|actual| argv_matches(argv, actual))
            })
    }
}

fn argv_matches(expected: &[OsString], actual: &[OsString]) -> bool {
    let (Some((program, arguments)), Some((invocation, actual_arguments))) =
        (expected.split_first(), actual.split_first())
    else {
        return false;
    };
    arguments_equal(arguments.iter().map(OsString::as_os_str), actual_arguments)
        && (program == invocation
            || (!invocation.as_encoded_bytes().contains(&b'/')
                && Path::new(program).file_name() == Some(invocation.as_os_str())))
}

fn arguments_equal<'a>(
    expected: impl ExactSizeIterator<Item = &'a OsStr>,
    actual: &[OsString],
) -> bool {
    expected.len() == actual.len() && expected.zip(actual).all(|(left, right)| left == right)
}
