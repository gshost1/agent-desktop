I ran a read-only survey of 20 repos via `gh api` (source and docs), with nothing installed or run. Main finding: Cua's driver (MIT) already has most of what we lack: diff reads, a narrow default view, menu-shortcut mapping and resolution of out-of-process Save panels. Peekaboo (MIT) has the most complete file-dialog flow, but it runs in the foreground. No tool does Go to Folder headless with a verified result.

## 1. Ranked ideas

Impact numbers are my estimates, not measured.

1. **Diff reads after an action.** Cua `get_window_state` takes `since:"latest"` or a snapshot id and returns only `+` added, `~` changed and `-` removed rows, or `no change; focused element is …`. mcp-server-macos-use returns only the diff from click/type/press, and Terminator has `ui_diff_before_after`.
   - Levers: turns and observation size.
   - Evidence: `trycua/cua` `libs/cua-driver/rust/crates/platform-macos/src/tools/get_window_state.rs` (tool description); `mediar-ai/mcp-server-macos-use` `Sources/MCPServer/main.swift` (`EnrichedTraversalDiff`); `mediar-ai/terminator` `crates/terminator/src/ui_tree_diff.rs`.
   - Impact: removes most "snapshot again" turns; maybe 25–40% fewer turns and tokens.
   - License: Cua and Terminator are MIT. mcp-server-macos-use is **BSL-1.1**, so take the idea only, not the code. Application-agnostic.

2. **Write the observation to a file and print a short summary.** Playwright CLI prints only `[Snapshot](.playwright-cli/page-….yml)`. mcp-server-macos-use prints status, file path, size and a ready `grep -n 'AXButton' <file>` hint. Chrome DevTools MCP's design principle is "Files are the right location for large amounts of data."
   - Lever: observation size. It fits us well because our agent already works through Bash.
   - Evidence: `microsoft/playwright-cli` README "Snapshots"; `main.swift` `buildCompactSummary`; `ChromeDevTools/chrome-devtools-mcp` `docs/design-principles.md`.
   - Impact: large on System Settings and Finder, where the agent reads only what it greps.
   - License: Apache-2.0 / MIT. Application-agnostic.

3. **Narrow by default.** Cua walks at most 250 nodes and says so when it truncates. Its `query` returns matching rows plus their ancestors. With no query, it lists only the menu bar's top-level titles (`collapse_menus`).
   - Lever: observation size.
   - Evidence: same `get_window_state.rs`, around lines 340–360.
   - Impact: medium; we already have `find` and `query`, so the new parts are ancestors-in-context and collapsed menus.
   - License: MIT. Application-agnostic.

4. **Turn a shortcut into a menu click.** Cua found that background key events never trigger an app's menu shortcuts: a background `cmd+s` to TextEdit "was reported as Pressed and saved nothing". Its `hotkey` looks up the menu item whose shortcut matches (`AXMenuItemCmdChar` and modifiers) and presses it through accessibility, or refuses if the item is disabled.
   - Levers: makes hard headless cases work; fewer turns.
   - Evidence: `platform-macos/src/tools/menu_equivalent.rs`.
   - Impact: this likely explains the silent Cmd+Shift+G and Cmd+Shift+N failures in Finder, and affects TextEdit save.
   - License: MIT. Application-agnostic.

5. **Find the real owner of Open/Save panels.** A sandboxed app's panel window belongs to `com.apple.appkit.xpc.openAndSavePanelService`, not to the app. Cua refuses with `window_owner_pid_mismatch` and names the real owner pid (its issue #2237, TextEdit). mcp-server-macos-use automatically reads the new app when the frontmost app changes after an action, and scopes reads to an `AXSheet`.
   - Levers: makes hard cases work; turns.
   - Evidence: `platform-macos/src/windows.rs` (`ForeignPid`); `main.swift` `findSheetBounds`.
   - License: MIT (Cua). Application-agnostic, because it uses WindowServer ownership, not app names.

6. **One command for the file dialog.** Peekaboo's `dialog file --path --name` sets the name field (`AXIdentifier == "saveAsNameTextField"`). If the panel is collapsed it expands it. It then sets `PathTextField` and presses Return, falls back to Go to Folder (Cmd+Shift+G, type, Return), and checks the value afterwards. These identifiers come from AppKit's shared Save panel, not from one app.
   - Lever: turns; the save-dialog task could drop to 1–2 turns.
   - Evidence: `openclaw/Peekaboo` `Core/PeekabooAutomationKit/Sources/PeekabooAutomationKit/Services/UI/DialogService+FileDialogNavigation.swift` and `DialogService+FileDialogFilename.swift`.
   - Caveat: it activates the app and uses global input.
   - License: MIT. Application-agnostic.

7. **Batch with stop conditions.** browser-use `multi_act` stops the rest of the queue when the URL or focused target changes, or after an action flagged `terminates_sequence`. UFO2 predicts several actions in one call, checks them against the live UI and reports "up to 51% fewer LLM calls". macOS-use batches up to 10 actions on stable UIs such as Calculator.
   - Lever: turns.
   - Evidence: `browser-use/browser-use` `browser_use/agent/service.py:2755`; `microsoft/UFO` `documents/docs/ufo2/core_features/multi_action.md`; `browser-use/macOS-use` `mlx_use/agent/prompts.py`.
   - Adds to our batch: stop when a sheet or window appears or focus moves, and return one diff at the end.
   - License: MIT. Application-agnostic.

8. **Wait for the UI to settle after an action.** Chrome DevTools MCP waits up to 100 ms for a navigation to start, then until the page has been quiet for 100 ms, capped at 3 s (`WaitForHelper.ts`). The accessibility equivalent is a quiet period on AX notifications. The opposite example is Anthropic's demo, which sleeps a fixed 2.0 s before each screenshot (`computer-use-demo/computer_use_demo/tools/computer.py`).
   - Levers: latency and turns.
   - License: Apache-2.0. Application-agnostic.

9. **Cheaper serialization.** Drop a static-text child when its text equals the parent's name (Chrome DevTools MCP `src/formatters/SnapshotFormatter.ts`, `#hasRedundantTextChildren`). Use a pipe-separated table with a header row (Windows-Use `windows_use/agent/tree/views.py`). Use one-line rows like `- [N] AXButton "t" = "v" actions=[…]` (Cua `ax/tree.rs`). Leave ids and bounds out of diffs (Terminator).
   - Lever: observation size, about 10–20% on top of our outline format.
   - License: Apache-2.0 / MIT. Application-agnostic.

10. **Background clicks into a specific window.** Cua posts mouse events with SkyLight `SLEventPostToPid`, falling back to the public `CGEvent::post_to_pid`. It stamps event fields f40=pid, f51/f91/f92=window id and f58=click group, plus the window-local point. macOS 27 treats that stamp as window-local, so the code branches on OS version.
    - Lever: makes headless clicks work in Chromium and Catalyst apps.
    - Evidence: `platform-macos/src/input/mouse.rs`, around lines 1530–1560 and 570–600.
    - Risk: private API. License: MIT. Application-agnostic.

11. **A persistent helper that keeps state warm.** Peekaboo's daemon keeps snapshots, window tracking and AX observers alive between calls and reports `used_cache` (`docs/daemon.md`, `docs/MCP.md`). Cua starts the screenshot while it walks the tree (`prefetch_capture`).
    - Lever: latency only; small next to model time.
    - License: MIT.

12. **A smaller fixed prompt.** Playwright says a CLI plus skills uses fewer tokens than MCP schemas, and Chrome DevTools MCP has a `--slim` tool set. For us: keep the always-loaded skill short and move rarely used sections into on-demand `skills get` output.
    - Lever: fixed prompt.

13. **Probe an app for a structural channel first.** huashu-mac-use runs `probe.sh` to find an AppleScript dictionary, URL scheme or debug (CDP) port, uses that channel first and falls back to accessibility. It already uses agent-desktop for its accessibility layer.
    - Lever: turns.
    - Evidence: `alchaincyf/huashu-mac-use` README.
    - License: MIT. Application-agnostic, since it detects capabilities.

Left out as poor fits for this benchmark:
- Replaying recorded or compiled runs (Stagehand cache, OpenAdapt, Terminator workflows). They save a lot but would game a fresh-agent benchmark.
- Vision and set-of-mark tools (OmniParser, UI-TARS). They add tokens.
- Skyvern: AGPL.

## 2. Finder rename, Go to Folder and Save-panel folder

- **Rename.** Cua confirms our finding in code. Setting AXValue on a Finder file-name cell "takes an AXValue write and reads it back, but the file is never renamed", and the same is true of the Get Info Name field. Setting it on the *inline rename editor while it is focused* does work (`set_value.rs`, around lines 138–160 and 286–330).
  - Cua's fix renames through Finder's scripting interface, keyed on bundle id `com.apple.finder`. That is an app-specific branch, which our rules forbid.
  - Options that stay app-agnostic: open the inline editor first, then write to it; or rename on disk for any element whose `AXURL` is a `file://` path and confirm with a fresh read. Neither is tested.
- **Go to Folder and the Cmd shortcuts.** Idea 4 is the likely root cause: background keys never trigger menu shortcuts, so Cmd+Shift+G and Cmd+Shift+N need to become an `AXPress` on the menu item.
- **Save panel.** Idea 5: in sandboxed apps such as TextEdit, the panel belongs to the panel-service process. My guess, not verified, is that our Return goes to TextEdit's pid and never reaches the panel.
  - Peekaboo's flow (idea 6) works, but in the foreground.
  - Ideas to try headless, none confirmed by any tool: the `AXConfirm` action on the field, Return posted to the panel-service pid, or selecting a sidebar row.
- **Bottom line: no surveyed tool does any of these three headless with a verified effect.**

## 3. Published numbers

| Tool | Published claim | Source |
|---|---|---|
| UFO2 | Up to 51% fewer LLM calls; "10%+ better success" | `documents/docs/ufo2/overview.md` |
| Agent S3 / S4 | OSWorld 72.6% (S3); OSWorld V2 21.3% with Qwen3.8-27B (S4) | Agent-S README, `agent_s4/README.md` |
| Skyvern | WebVoyager 85.8%; WebBench 64.4% | README |
| OmniParser v2 | ScreenSpot Pro 39.5% | README |
| Terminator | ">95% success", "100x faster" (marketing, no method given) | README |
| Cua Driver Bench | Harness only; "No results are published here yet" | `libs/cua-driver-bench/README.md` |
| Peekaboo, macOS-use, Windows-Use, Playwright MCP, Chrome DevTools MCP, Stagehand | None found | — |

## 4. Not confirmed

- browser-use's WebVoyager number and UI-TARS's OSWorld numbers: not checked.
- The license of Open Interpreter's old OS mode (tag v0.4.2). The current repo reports Apache-2.0 and no longer has OS mode, but older versions may have been AGPL.
- OmniParser's license: GitHub reports CC-BY-4.0 for the repo, while the README badge says MIT and older icon-detector weights are AGPL.
- Terminator is now Windows-only, and its macOS claims are gone.
- All the Save-panel headless ideas, and whether diff reads beat our outline format on the real benchmark apps.

Licenses: Cua, Peekaboo, AXorcist, Terminator, macOS-use, UFO, OpenAdapt, Windows-Use, Stagehand, claude-quickstarts, macos-automator-mcp and huashu-mac-use are MIT. Playwright MCP, playwright-cli, Chrome DevTools MCP, UI-TARS-desktop and Agent-S are Apache-2.0. All of these are compatible with our Apache-2.0. mcp-server-macos-use (BSL-1.1) and Skyvern (AGPL-3.0) are not; take ideas only.

