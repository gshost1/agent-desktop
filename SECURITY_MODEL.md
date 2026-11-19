# Security model: agent-desktop and tabref

This threat model covers both gshost1/agent-desktop and gshost1/tabref, because tabref reuses code extracted from agent-desktop's web crate (see tabref `NOTICE`). The same file lives in the root of both repositories. Security scans start from it; update it when the trust boundaries change.

Sources: agent-desktop `SECURITY.md` (report scope: CLI, npm installer, FFI boundary, release artifacts, GitHub Actions workflows, platform adapters) and the maintainer's decisions, marked **(owner)**. Everything else is read from the code.

Infrastructure **(owner)**: none. These are local tools with no infrastructure repository.

Scan scope **(owner)**: shipped code only — `src/`, `crates/`, `npm/`, `skills/`, `.github/workflows/`; probes, benchmarks, `apps/docs` and tests are skipped.

## System overview

Both repositories are **local tools an AI agent invokes on the user's own machine**. Neither runs a hosted service or has infrastructure-as-code. The observe-act loop and any LLM live in the calling agent.

- **agent-desktop** (Rust CLI + C ABI library, fork of vercel-labs/agent-desktop): reads native accessibility trees (macOS AX, Windows UIA, Linux AT-SPI over D-Bus), returns JSON with refs, and drives apps (click, type, keyboard/mouse/clipboard). A `web` surface reads/drives Chrome pages over the DevTools Protocol. Also exposes an MCP server (`agent-desktop mcp`) that re-runs the binary per tool call. Ships as GitHub release archives and an npm package whose `postinstall` downloads the binary.
- **tabref** (Rust CLI): reads and automates Chrome pages over CDP. Code derived from agent-desktop's `crates/web` (see tabref `NOTICE`). Runs a per-user background daemon over a Unix socket, can launch Chrome with a remote-debugging port and a persistent profile, manages downloads (ledger, `tend`), uploads, screenshots, `js` evaluation and multi-step `do` batches, with an "app guard" for navigation/close protections.

## Components

| Name | Repo | Code |
|---|---|---|
| CLI | agent-desktop | `src/` (cli, cli_args, dispatch, batch, command_policy, diagnostic, visual_debug) |
| MCP server | agent-desktop | `src/mcp/` |
| Core & state | agent-desktop | `crates/core/` (sessions, interaction lease, storage/private files, refs, trace) |
| Platform adapters | agent-desktop | `crates/macos/`, `crates/windows/`, `crates/linux/` |
| FFI | agent-desktop | `crates/ffi/` (cdylib, `crates/ffi/include/agent_desktop.h`) |
| Web surface (CDP) | agent-desktop | `crates/web/`, `crates/core/src/system/cdp_endpoint.rs` |
| npm installer | agent-desktop | `npm/` (`scripts/postinstall.js`, `bin/`) |
| Release pipeline | both | `.github/workflows/`, `scripts/` |
| tabref CLI & daemon | tabref | `crates/tabref/` (commands, daemon, session) |
| tabref CDP & launch | tabref | `crates/tabref-cdp/` (discovery, launch, transport, target) |
| tabref page engine | tabref | `crates/tabref-page/`, `crates/tabref-core/` |

## Trust boundaries and entry points

1. **Calling agent -> CLI/MCP/FFI.** argv, batch JSON on stdin, MCP tool calls, C ABI calls. **(owner)** Not trusted: the agent may be prompt-injected.
2. **Untrusted content -> tool.** Accessibility trees of arbitrary apps, web page DOM/AX trees, page-controlled strings (titles, names, URLs, download filenames, dialog text) flow into JSON output, file paths and logs. This is the main untrusted input.
3. **CDP endpoint.** Discovery reads `DevToolsActivePort` / HTTP `json/version` from a local port; anything listening on that port is treated as the browser. tabref may launch Chrome with `--remote-debugging-port`.
4. **Other local users/processes.** State roots (`~/.agent-desktop`, `TABREF_HOME`), tabref daemon socket dir under `$XDG_RUNTIME_DIR`/`$TMPDIR`/`/tmp` named `tabref-<uid>-<hash>`, lock files, macOS cursor-overlay helper endpoint, screenshots and traces on disk.
5. **Supply chain.** npm `postinstall` downloads from GitHub releases with checksum verification; release workflows publish artifacts.
6. **FFI boundary.** Native callers pass pointers/handles into the cdylib (memory safety, panics across FFI).

## Assets

Most critical **(owner)**: the user's files (downloads, uploads, screenshot paths, state directories) and the user's logged-in browser sessions/cookies reachable over CDP.

Also in play (inferred):

- User's logged-in browser sessions (cookies, tokens) reachable through CDP — full account takeover if the port is reachable by others.
- Private desktop and page content in snapshots, screenshots, clipboard reads, traces, debug artifacts (README warns about this).
- User's files: download destinations, upload sources, screenshot paths, state directories.
- Desktop input control (synthetic keyboard/mouse) — whoever controls it acts as the user.
- Integrity of release binaries and the npm package.

## Attacker profiles **(owner)**

All of these are in scope:

- Malicious web page or app UI content the agent reads or acts on.
- The calling AI agent itself, prompt-injected into issuing harmful commands. The agent is therefore **not** a trusted boundary: argv, batch/`do` JSON, MCP tool calls and FFI input are attacker-influenced.
- A malicious or spoofed Chrome DevTools endpoint.
- Other local users or processes on the same machine (sockets, state files, CDP port).
- Supply chain: release artifacts, the npm installer, GitHub Actions.

## Existing controls

- Private-file helpers and state-exposure checks (`crates/core/src/storage/private_file*.rs`, `state_exposure.rs`; tabref `tabref-core/src/storage/private_file.rs`).
- Per-user private daemon socket directory (`tabref/src/daemon/private_dir.rs`, `daemon_address.rs`).
- CDP listener ownership checks (`tabref-cdp/src/discovery/listener_owner.rs`, `lsof_owner.rs`, `tabref/src/session/endpoint_refusal.rs`).
- Command policy preflight (`src/command_policy/`), interaction lease for desktop input, headed-mode gating of physical input.
- App guard for navigation/close (`tabref/src/commands/app_guard.rs`).
- npm installer checksum verification; `permissions: {}` default in release workflow.

## Out of scope / accepted risk

- **(owner)** Probes, benchmarks and test harnesses (`probes/`, `benchmarks/`, `bench/`, `tests/`, `*_tests.rs`).
- Explicitly requested JS evaluation, typing, clicking and uploads are **not** accepted risk **(owner)**. Because a prompt-injected agent is in scope, harm reachable through those commands still counts.

## Open questions

- Is the agent-desktop docs site (`apps/docs`) in scope for a later scan? Current scans skip it.
- Platform priority: are all three OSes shipped equally, or is macOS primary?
