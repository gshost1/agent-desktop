# deskref

[![npm](https://img.shields.io/npm/v/deskref)](https://www.npmjs.com/package/deskref)

Computer use for AI agents on macOS, Windows, and Linux. It reads the accessibility tree instead of screenshots, and it's faster and cheaper than upstream [agent-desktop](https://github.com/vercel-labs/agent-desktop).

## What it is

deskref is a fork of vercel-labs/agent-desktop. It reads an app's UI through the OS accessibility tree and returns JSON with a ref for each element an agent can act on. The agent acts on a ref, then reads the UI again to verify the change. There is no model inside; the loop lives in your agent, and deskref ships as a CLI and a C library.

## Why use it

- **Faster.** 186 s vs 439 s for upstream on the Mac dev set (fixes were tuned on it; about 2.4× faster), and faster than Peekaboo (264 s) and Cua Driver (316 s).
- **Fewer tokens.** 637k tokens vs 891k for upstream (29% fewer), 922k for Peekaboo, and 1623k for Cua Driver.
- **More tasks passed.** 48/50 runs passed on the Mac dev set, compared with 46/50 for Peekaboo and 44/50 for upstream and Cua Driver (not statistically significant; see below).
- **Web pages in Chrome through the same loop.** `snapshot --surface web` reads Chrome over DevTools with the same refs as native apps, cutting upstream's web wall time by 64% on Mac and 88% on Linux on held-out web tasks (2026-11-11 run).

https://github.com/user-attachments/assets/aa952405-68c1-487a-965e-ab757f130110

## Mac dev set benchmark (2026-11-22)

10 macOS native tasks, 5 runs each, using Claude Sonnet 5.5 via Claude Code. The tasks are private, and 1.6.0's fixes were tuned on them, so these numbers show progress on a known set, not how the tools do on unseen tasks. Each figure is the sum of per-task medians.

| Tool | Runs passed | Wall time | Tokens |
|---|---|---|---|
| deskref 1.6.0 (fork `f50cdf6`) | 48/50 | 186 s | 637k |
| upstream agent-desktop (`8e8dd7cbe`) | 44/50 | 439 s | 891k |
| Peekaboo 4.9.8 | 46/50 | 264 s | 922k |
| Cua Driver 0.47.7 | 44/50 | 316 s | 1623k |

deskref is about 2.4× faster than upstream with 29% fewer tokens. Its pass-rate lead isn't statistically significant (Holm-corrected p ≈ 0.66).

## Install

Install from npm:

```bash
npm install -g deskref
deskref status                # platform, permissions, supported surfaces
deskref permissions --guide   # macOS: walks you through the Accessibility grant
```

`npm install -g deskref` installs one executable under two command names, `deskref` and `agent-desktop`, and every example below works with either. The `agent-desktop` npm package and `brew install agent-desktop` install upstream's build, which has no web surface.

Or download the archive for your OS and architecture from the [latest release](https://github.com/gshost1/agent-desktop/releases/latest), and extract the executable and its helper into one directory on your `PATH`. On macOS and Linux:

```bash
VERSION=1.5.0
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

## Quick start

```bash
deskref snapshot --app Finder -i      # interactive elements, each with a ref
deskref click @s8f3k2p9:e8            # act on a ref from that snapshot
deskref snapshot --app Finder -i      # observe again to verify
```

To see a visible cursor for each action, run `deskref session start --cursor` and export its `AGENT_DESKTOP_SESSION` (printed in `data.next`).

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
- **Select a row on macOS.** Pass the table or outline row's ref and exact label to `select`. This can replace an
  existing selection. [Select command](https://agent-browser.dev/desktop/commands/interaction#select).
- **Post-action waits** observe the ref's original process. Window-owned refs stay pinned to their original window,
  including refs captured from a sheet, alert, popover, focused view, or Linux menu. macOS application menus keep
  their application-owned surface and need no document window. The wait reads the whole pinned window, so overlay
  selectors share its scope. [Wait scope and results](https://agent-browser.dev/desktop/configuration#post-action-wait-flags)
  describe the selector limits.
- `type`, `set-value`, and `clipboard-set` accept `--stdin` for piped text, keeping it out of process arguments. Run secret steps as separate commands; batch JSON text appears in process arguments.
- **PowerShell.** Quote every ref (`'@s8f3k2p9:e8'`), because a bare `@token` is the splatting operator.
- **Privacy.** Snapshots, clipboard reads, screenshots, command output, and debug artifacts can contain private desktop content. Keep them out of public logs and uploads.

The full walkthrough is in the [quick start](https://agent-browser.dev/desktop/quick-start).

## What this fork changes

- **Web pages.** `snapshot --surface web` reads a page in Chrome over DevTools and assigns refs, so the same snapshot-and-ref loop drives web pages and native apps. Upstream has no web surface and reads Chrome through the accessibility tree. `deskref skills get web` is the guide.
- **Faster macOS commands.** Every command is a new process. 1.4.0 opens the window-server and accessibility connections while permission checks run, and reads an element's children in one call instead of three. On System Settings, median `find --text` fell from 992 ms to 296 ms.
- **Fewer tokens per turn.** Global `--lines` prints one line per node instead of a JSON tree, about a third fewer tokens. The web guide an agent loads first is 1,818 tokens.

The [changelog](CHANGELOG.md) lists every change with its measurement.

## Earlier benchmarks

### 1.4.0 suite (2026-11-10)

Claude Opus 5.5 via `claude -p`, Bash only, each tool's own skill, on one MacBook Air (M2, 8 GB), with a script judging every run. Each figure is the sum of per-task medians (other tool → this fork).

| Tasks | Compared with | Wall time | Input tokens | Runs passed |
|---|---|---|---|---|
| 3 native app tasks, 8 runs each | upstream | 68.6 s → 37.8 s (-45%) | 190K → 96K (-49%) | 24/24 → 24/24 |
| 6 harder native app tasks, 4 runs each | upstream | 182.4 s → 101.8 s (-44%) | 474K → 292K (-38%) | 23/24 → 24/24 |
| 9 web tasks, 3 runs each | upstream | 655.7 s → 232.3 s (-65%) | 1.96M → 475K (-76%) | 27/27 → 26/27 |
| 9 web tasks, 3 runs each | browser-harness 0.1.13 | 224.0 s → 184.3 s (-18%) | 366K → 396K (+8%) | 27/27 → 27/27 |

Upstream is `8e8dd7cbe`; this fork is `697c618d6` (1.4.0). On web tasks, upstream reads Chrome through the accessibility tree while this fork uses `--surface web`; against browser-harness, this fork is faster with similar token use. One model, one machine, 3 to 8 runs per task; see [pull request #131](https://github.com/gshost1/agent-desktop/pull/131) for the task list, per-benchmark tables, and limits.

### Held-out tasks on Mac and Linux (2026-11-11)

The fork was tuned on the 1.4.0 tasks above. This held-out run tested 18 unseen tasks frozen from public benchmarks: 8 OSWorld desktop tasks, 7 local WebArena tasks, and 3 WebVoyager tasks (arXiv, Cambridge Dictionary). Each tool ran each task 5 times with Claude Sonnet 5.5 via Claude Code on a MacBook Pro (M1 Pro, macOS 15; TextEdit, Preview, System Settings) and on Linux VMs (LibreOffice Writer, GIMP, GNOME Settings). Tool time is time inside tool commands; wall time is the whole run. Each figure is the sum of per-task medians (best other tool → this fork).

| Suite | Tool time | Wall time | Runs passed |
|---|---|---|---|
| Mac, 8 native tasks | Cua Driver 52.4 s → 36.7 s | Cua Driver 277.7 s → 368.5 s | Peekaboo 31/40 → 32/40 |
| Mac, 10 web tasks | agent-browser 7.7 s → 10.3 s | browser-harness 111.0 s → 132.2 s | browser-harness 50/50 → 47/50 |
| Linux, 8 native tasks | upstream 102.1 s → 96.9 s | upstream 318.4 s → 371.4 s | upstream 36/40 → 38/40 |
| Linux, 10 web tasks | agent-browser 5.5 s → 8.2 s | browser-harness 110.4 s → 121.9 s | browser-harness 50/50 → 46/50 |

This fork had the lowest tool time in both native suites and the lowest wall time in none; tool time was 7% to 26% of wall time, with model turns taking the rest. On the web, agent-browser had lower tool time on both machines, while this fork passed more runs on Mac (47/50 vs 43/50) and tied on Linux (46/50); against browser-harness, this fork's web tool time was 41% lower on Mac and 52% lower on Linux. Against upstream on the web, it cut tool time by 77% on Mac and 97% on Linux, and wall time by 64% and 88%. On Linux native tasks it was 17% slower than upstream end to end and used 12% more tokens.

Losses and limits:

- On Mac it passed 0/5 runs of a TextEdit strike-through task (upstream also 0/5; Peekaboo and Cua Driver 3/5).
- On both machines it miscounted the meanings of a Cambridge Dictionary entry (3/5 runs on Mac, 4/5 on Linux).
- Compared tools: upstream `8e8dd7cbe`, Peekaboo 4.9.8 (macOS only), Cua Driver 0.47.7 (Mac) and 0.46.1 (Linux), browser-harness 0.1.13, agent-browser 0.42.0 (run after the others on the same tasks and seeds), and Playwright CLI 0.1.24 (Mac). Compare within one machine, as native tasks used different apps per OS. On Linux, the harness's command check failed 2 correct runs of this fork, counted as failures above.

## How it works

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/architecture-dark.png">
  <img alt="The calling agent runs the agent-desktop CLI or the C library. Both go through agent-desktop-core and the PlatformAdapter trait to one platform crate, chosen at compile time, which reads the target app through the AX API on macOS, UI Automation on Windows, or AT-SPI on Linux." src="assets/architecture-light.png" width="720">
</picture>

Your agent calls the CLI, or the C library in process. Both run the same core, which reaches the OS only through `PlatformAdapter`. Each binary is built for one OS and talks to that OS's accessibility API.

## For AI agents

The binary carries guides that match its version:

```bash
agent-desktop skills get desktop      # the loop, refs, read-back and error recovery; topics hold the rest
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
