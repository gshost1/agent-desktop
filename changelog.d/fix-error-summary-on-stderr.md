### Bug Fixes

* An error envelope now also prints one line on stderr, `agent-desktop: <CODE>: <message> -- <suggestion>`, and a successful `batch` prints one per failed entry, so an agent that pipes output into `jq`, `grep` or `head` still sees why the command failed instead of `jq: Cannot iterate over null` (10-20 such hidden errors per task in Linux bench transcripts). Stdout and exit codes are unchanged; `AGENT_DESKTOP_STDERR_SUMMARY=0` turns the line off. ([#168](https://github.com/gshost1/agent-desktop/pull/168))
