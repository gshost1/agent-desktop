# agent-desktop

Computer use for AI agents: see and operate desktop apps on macOS, Windows, and Linux through the OS accessibility tree.

This repository is a fork of [vercel-labs/agent-desktop](https://github.com/vercel-labs/agent-desktop). It adds a web surface for pages in Chrome and makes macOS commands faster. It ships as [GitHub release archives](https://github.com/gshost1/agent-desktop/releases); the `agent-desktop` packages on npm and Homebrew are upstream's builds.

agent-desktop gives an AI agent computer use without screenshots or pixel matching. It reads the accessibility tree of a desktop app and returns it as JSON, with a ref for every element the agent can act on. The agent clicks or types on a ref, reads the UI again to check the result, and repeats. The loop lives in your agent; agent-desktop has no model inside. It ships as a CLI and a C library.

Snapshots, clipboard reads, screenshots, command output, and debug artifacts can contain private desktop content. Keep them out of public logs and uploads.

## What this fork changes

- **Web pages.** `snapshot --surface web` reads a page in Chrome through the browser's DevTools endpoint and gives its elements refs, so the same snapshot-and-ref loop drives a web page and a native app. Upstream has no web surface and reads Chrome through the accessibility tree. `agent-desktop skills get web` is the guide.
- **Faster macOS commands.** Every command is a new process. 1.4.0 opens the window-server and accessibility connections while the permission checks run, and reads an element's children with one call instead of three. On System Settings, the median `find --text` fell from 992 ms to 296 ms.
- **Fewer tokens per turn.** The global `--lines` flag prints one line per node in place of the JSON tree, about a third fewer tokens. The web guide an agent loads first is 1,818 tokens.

The [changelog](CHANGELOG.md) lists every change with its measurement.

## Benchmarks

An agent ran the same tasks with each tool, and a script judged every run against the expected result. All runs used Claude Opus 5.5 through `claude -p` with Bash only and each tool's own skill, on one MacBook Air (M2, 8 GB), on 2026-11-10. Each figure is the sum of the per-task medians, shown as the other tool, then this fork.

| Tasks | Compared with | Wall time | Input tokens | Runs passed |
|---|---|---|---|---|
| 3 native app tasks, 8 runs each | upstream | 68.6 s → 37.8 s (-45%) | 190K → 96K (-49%) | 24/24 → 24/24 |
| 6 harder native app tasks, 4 runs each | upstream | 182.4 s → 101.8 s (-44%) | 474K → 292K (-38%) | 23/24 → 24/24 |
| 9 web tasks, 3 runs each | upstream | 655.7 s → 232.3 s (-65%) | 1.96M → 475K (-76%) | 27/27 → 26/27 |
| 9 web tasks, 3 runs each | browser-harness 0.1.13 | 224.0 s → 184.3 s (-18%) | 366K → 396K (+8%) | 27/27 → 27/27 |

Upstream is its main branch at `8e8dd7cbe`. This fork is `697c618d6`, the code released as 1.4.0. The web comparison with upstream sets two paths to the same task side by side: upstream reads Chrome through the accessibility tree, and this fork uses `--surface web`. Against browser-harness, this fork is faster and uses about the same number of tokens.

These results come from one model on one machine, with 3 to 8 runs per task. The [1.4.0 release pull request](https://github.com/gshost1/agent-desktop/pull/131) has the task list, the per-benchmark tables, and the limits.

### Held-out tasks on Mac and Linux

The fork was tuned on the tasks above. A second benchmark checks it on tasks it never saw: 18 tasks adapted from public benchmarks and frozen before the first run, namely 8 OSWorld desktop tasks, 7 WebArena tasks on a local copy of their sites, and 3 WebVoyager tasks on arXiv and Cambridge Dictionary. Each tool ran each task 5 times with Claude Sonnet 5.5 through Claude Code on 2026-11-11, on a MacBook Pro (M1 Pro, macOS 15) and on Linux virtual machines. The Mac ran the native tasks in TextEdit, Preview and System Settings, and Linux ran them in LibreOffice Writer, GIMP and GNOME Settings, as OSWorld does. Tool time is the time spent inside the tool's own commands, and wall time is the whole run. Each figure is the sum of the per-task medians, shown as the best other tool, then this fork.

| Suite | Tool time | Wall time | Runs passed |
|---|---|---|---|
| Mac, 8 native tasks | Cua Driver 52.4 s → 36.7 s | Cua Driver 277.7 s → 368.5 s | Peekaboo 31/40 → 32/40 |
| Mac, 10 web tasks | agent-browser 7.7 s → 10.3 s | browser-harness 111.0 s → 132.2 s | browser-harness 50/50 → 47/50 |
| Linux, 8 native tasks | upstream 102.1 s → 96.9 s | upstream 318.4 s → 371.4 s | upstream 36/40 → 38/40 |
| Linux, 10 web tasks | agent-browser 5.5 s → 8.2 s | browser-harness 110.4 s → 121.9 s | browser-harness 50/50 → 46/50 |

This fork had the lowest tool time in both native suites and the lowest wall time in none. On the web, agent-browser's tool time was lower on both machines, and this fork passed more runs on the Mac (47/50 against 43/50) and the same number on Linux (46/50). Against browser-harness, this fork's web tool time was 41% lower on the Mac and 52% lower on Linux. Its tool time was 7% to 26% of its wall time, and the model's turns took the rest, so a faster tool moves the total only a little. On the web it cut upstream's tool time by 77% on the Mac and 97% on Linux, and its wall time by 64% and 88%. On the Linux native tasks it was 17% slower than upstream end to end and used 12% more tokens.

It fell short on two tasks. On the Mac it passed none of 5 runs of a TextEdit strike-through task, which upstream also failed and Peekaboo and Cua Driver passed 3 times out of 5. On both machines it miscounted the meanings of a Cambridge Dictionary entry, in 3 of 5 runs on the Mac and 4 of 5 on Linux.

The other tools were upstream at `8e8dd7cbe`, Peekaboo 4.9.8, Cua Driver 0.47.7, browser-harness 0.1.13, agent-browser 0.42.0 and Playwright CLI 0.1.24 on the Mac. agent-browser ran on both machines after the other tools, on the same tasks with the same seeds. Linux has no Peekaboo, which runs only on macOS, and used Cua Driver 0.46.1. Compare tools within one machine, because the native tasks used different apps on each. On Linux the harness's command check also failed 2 correct runs of this fork, and the table counts them as failures.

## Install

Download the archive for your OS and architecture from the [latest release](https://github.com/gshost1/agent-desktop/releases/latest), and extract the executable and its helper into one directory on your `PATH`. On macOS and Linux:

```bash
VERSION=1.4.0
TARGET=aarch64-apple-darwin         # or x86_64-apple-darwin, aarch64-unknown-linux-gnu, x86_64-unknown-linux-gnu
BASE="https://github.com/gshost1/agent-desktop/releases/download/v$VERSION"
curl -fLO "$BASE/agent-desktop-v$VERSION-$TARGET.tar.gz"
curl -fLO "$BASE/checksums.txt"
shasum -a 256 -c checksums.txt --ignore-missing     # Linux: sha256sum -c checksums.txt --ignore-missing
mkdir -p ~/.local/bin
tar -xzf "agent-desktop-v$VERSION-$TARGET.tar.gz" -C ~/.local/bin
agent-desktop status                # platform, permissions, supported surfaces
agent-desktop permissions --guide   # macOS: walks you through the Accessibility grant
```

If `~/.local/bin` is not on your `PATH`, add it or extract into a directory that is. On Windows, download the `x86_64-pc-windows-msvc` or `aarch64-pc-windows-msvc` archive from the release page and extract it with `tar -xzf`.

This fork does not publish to npm or Homebrew. `npm install -g agent-desktop` and `brew install agent-desktop` install upstream's build, which has no web surface.

## Quick start

```bash
agent-desktop snapshot --app Finder -i      # interactive elements, each with a ref
agent-desktop click @s8f3k2p9:e8            # act on a ref from that snapshot
agent-desktop snapshot --app Finder -i      # observe again to verify
```

Each command prints one JSON envelope. Add `--lines` to get the tree as `data.lines`, one line per node with its ref first. A trimmed snapshot:

```json
{
  "version": "2.7",
  "ok": true,
  "command": "snapshot",
  "data": {
    "app": "Finder",
    "snapshot_id": "s8f3k2p9",
    "ref_count": 59,
    "complete": true,
    "tree": {
      "role": "window",
      "name": "Downloads",
      "children": [
        { "role": "treeitem", "name": "Documents", "ref_id": "@s8f3k2p9:e8", "available_actions": ["Click"] }
      ]
    }
  }
}
```

- **Refs belong to their snapshot.** A bare `@e8` is rejected with `INVALID_ARGS`. When the UI changes, an action
  returns `STALE_REF` or `AMBIGUOUS_TARGET`: take a new snapshot and use its refs.
- **Several windows** give `AMBIGUOUS_TARGET`; `snapshot` lists the candidate window ids in `error.details`. Pass one
  with `--window-id`.
- **Dense apps:** map first with `snapshot --skeleton --app Slack -i --compact`, then drill into one region with
  `snapshot --root @s8f3k2p9:e3 -i --compact`.
- **Headless by default.** Ref actions use the accessibility API without moving the pointer or taking focus. The global
  `--headed` flag allows physical input.
- **Post-action waits** observe the ref's original process. Window-owned refs stay pinned to their original window,
  including refs captured from a sheet, alert, popover, focused view, or Linux menu. macOS application menus keep
  their application-owned surface and need no document window. The wait reads the whole pinned window, so overlay
  selectors share its scope. [Wait scope and results](https://agent-browser.dev/desktop/configuration#post-action-wait-flags)
  describe the selector limits.
- `type`, `set-value`, and `clipboard-set` accept `--stdin` for piped text, keeping it out of process arguments. Run secret steps as separate commands; batch JSON text appears in process arguments.
- **PowerShell.** Quote every ref (`'@s8f3k2p9:e8'`), because a bare `@token` is the splatting operator.

The full walkthrough is in the [quick start](https://agent-browser.dev/desktop/quick-start).

## How it works

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/architecture-dark.png">
  <img alt="The calling agent runs the agent-desktop CLI or the C library. Both go through agent-desktop-core and the PlatformAdapter trait to one platform crate, chosen at compile time, which reads the target app through the AX API on macOS, UI Automation on Windows, or AT-SPI on Linux." src="assets/architecture-light.png" width="720">
</picture>

Your agent calls the CLI, or the C library in process. Both run the same core, which reaches the OS only through `PlatformAdapter`. Each binary is built for one OS and talks to that OS's accessibility API.

## For AI agents

The binary carries guides that match its version:

```bash
agent-desktop skills get desktop      # the loop, refs, verification and retry rules
agent-desktop skills get platform     # what differs on the OS you run on
agent-desktop skills get web          # web pages in Chrome; enough on its own for web work
agent-desktop <command> --help        # flags for one command
```

Install the skill into an agent harness from [skills.sh](https://skills.sh/vercel-labs/agent-desktop/agent-desktop):

```bash
npx skills add https://github.com/vercel-labs/agent-desktop --skill agent-desktop
```

Or add this to your `AGENTS.md`:

```markdown
Use `agent-desktop` to see and operate desktop apps. Before the first use, run
`agent-desktop skills get desktop` and `agent-desktop skills get platform`, and follow them.
```

Retry a failed command only when `error.disposition.retry` is `safe`. See [JSON output](https://agent-browser.dev/desktop/json-output) for the envelope, error codes, and exit codes.

### MCP

`agent-desktop mcp` serves every command as an MCP tool over stdio. A tool is named after its command (`desktop_snapshot`, `desktop_click`, `desktop_session_start`), takes that command's arguments, and returns its JSON envelope:

```json
{ "mcpServers": { "agent-desktop": { "command": "agent-desktop", "args": ["mcp"] } } }
```

Server-wide global flags given to `mcp`, such as `--session`, `--headed` or `--trace`, apply to every call. `mcp` refuses flags that describe a single command, such as `--debug` or `--wait-for`.

## Commands

agent-desktop has 61 command names, 57 operational commands. The other four (`key-down`, `key-up`, `mouse-down`, `mouse-up`) refuse to run until a daemon can release a held key or button. [Commands](https://agent-browser.dev/desktop/commands) documents each one.

| Group | Commands |
|---|---|
| Observe | `snapshot`, `find`, `get`, `is`, `screenshot`, `list-surfaces` |
| Act on a ref | `click`, `double-click`, `triple-click`, `right-click`, `type`, `set-value`, `clear`, `focus`, `select`, `toggle`, `check`, `uncheck`, `expand`, `collapse`, `scroll`, `scroll-to` |
| Keyboard and mouse | `press`, `hover`, `drag`, `mouse-move`, `mouse-click`, `mouse-wheel`, `key-down`, `key-up`, `mouse-down`, `mouse-up` |
| Apps and windows | `launch`, `close-app`, `list-apps`, `list-windows`, `list-displays`, `focus-window`, `resize-window`, `move-window`, `minimize`, `maximize`, `restore`, `open-system-surface`, `open-url` |
| Notifications | `list-notifications`, `dismiss-notification`, `dismiss-all-notifications`, `notification-action` |
| Clipboard | `clipboard-get`, `clipboard-set`, `clipboard-clear` |
| Wait | `wait` |
| Sessions | `session`, `trace`, `cursor-overlay` |
| System | `status`, `permissions`, `version`, `batch`, `skills` |

## Platform support

| Platform | Requirement | Accessibility API |
|---|---|---|
| [macOS](https://agent-browser.dev/desktop/platforms/macos) | macOS 13 or later, Intel or Apple Silicon | AX API |
| [Windows](https://agent-browser.dev/desktop/platforms/windows) | Windows 10 1809 or later, x64 or ARM64 | UI Automation |
| [Linux](https://agent-browser.dev/desktop/platforms/linux) | Ubuntu 24.04 or later, x86_64 or aarch64, X11 or Wayland | AT-SPI |

Each platform page lists what works headless, what needs `--headed`, and what refuses.

## C library

Each release ships `libagent_desktop_ffi`, so a program can call agent-desktop in process. See the [C library docs](https://agent-browser.dev/desktop/ffi).

```python
import ctypes
lib = ctypes.CDLL("./lib/libagent_desktop_ffi.dylib")
lib.ad_adapter_create.restype = ctypes.c_void_p
lib.ad_adapter_destroy.argtypes = [ctypes.c_void_p]
assert lib.ad_init(7) == 0  # AD_RESULT_OK when the header's AD_ABI_VERSION_MAJOR matches the library
adapter = lib.ad_adapter_create()
lib.ad_adapter_destroy(adapter)
```

## Development

Each platform adapter builds only on its own OS, so scope cargo to the host's packages. On macOS:

```bash
HOST_PKGS=(-p agent-desktop-core -p agent-desktop-macos -p agent-desktop -p agent-desktop-ffi)
cargo test "${HOST_PKGS[@]}"
cargo clippy "${HOST_PKGS[@]}" --all-targets -- -D warnings
```

On Windows, use `agent-desktop-windows` instead of `agent-desktop-macos`; on Linux, `agent-desktop-linux`. [CONTRIBUTING.md](CONTRIBUTING.md) lists every gate.

## Documentation

- [Introduction](https://agent-browser.dev/desktop): what it is and how it works
- [Snapshots and refs](https://agent-browser.dev/desktop/snapshots-and-refs)
- [Errors and recovery](https://agent-browser.dev/desktop/errors-and-recovery)
- [Skills](https://agent-browser.dev/desktop/skills)
- [CONCEPTS.md](CONCEPTS.md): vocabulary
- [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), [SUPPORT.md](SUPPORT.md)

## License

Apache-2.0
