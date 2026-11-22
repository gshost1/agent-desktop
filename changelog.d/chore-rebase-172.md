### Features

* Opt in to daemon mode with `AGENT_DESKTOP_DAEMON=1` or `--daemon` to reuse a warm background process and its connections between commands. The daemon serves one user and session on an owner-only socket and exits after `AGENT_DESKTOP_DAEMON_IDLE_MS` without a call (default 5 minutes). It accepts the next command while sending the previous reply and sends heartbeats while the command waits. Commands run in the CLI if the daemon cannot take them or the macOS caller lacks a required grant. Commands with uncertain delivery are never run twice. Available on Linux and macOS.
