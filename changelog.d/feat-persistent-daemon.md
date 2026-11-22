### Features

* Daemon mode: with `AGENT_DESKTOP_DAEMON=1` or `--daemon`, each call hands its command to a warm per-user, per-session background process of the same binary over an owner-only socket under the state root, so calls skip process and connection setup. Output, stderr and exit codes are the same as in-process. The first call starts the daemon; it exits after `AGENT_DESKTOP_DAEMON_IDLE_MS` without a call (default 5 minutes). A busy, hung, crashed or mismatched daemon falls back to running the command in the CLI, and a command that changes something is never run twice. Off by default; Linux and macOS, ignored on Windows.

### Bug Fixes

* Release the daemon's execution slot before sending a completed command's reply so the next call does not fall back to the CLI because the daemon still reports itself busy.
