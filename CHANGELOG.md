# Changelog

## Unreleased (1.4.0)

### BREAKING CHANGES

* The response envelope moves to 2.7. On Windows and Linux, and on macOS for an element with no address, a ref whose snapshot could not read the window whole now fails `STALE_REF` with `details.kind: identity_unproven` and `details.reason: capture_incomplete` instead of `AMBIGUOUS_TARGET` `identity_not_unique`; retake the snapshot with a larger `--timeout-ms`. A ref whose capture left a recorded field unread, or that no capture judged, fails `STALE_REF` `identity_unproven` with `details.reason: capture_unknown`. `identity_not_unique` now means the capture saw an identical twin. Refmap entries record `capture` (`unique`, `ambiguous`, `incomplete`, `unknown`); refmaps from earlier versions still load. See the version 2.7 migration in `docs/json-output.md`. ([#14](https://github.com/gshost1/agent-desktop/pull/14))

### Bug Fixes

* On macOS, `launch` reports its window once the window server has placed it, so `data.window` carries the real bounds and `visible: true`. It used to report the first record it read, which right after start-up had bounds 0x0 and `visible: false` (Calculator, the SwiftUI fixture, Script Editor and Weather, 16 of 16 launches). `launch` takes the time the window needs to reach the screen, read at most 50 ms apart. ([#100](https://github.com/gshost1/agent-desktop/pull/100))
* On Linux, a `launch` whose spawned process keeps running without joining the accessibility bus (a plain X11 program, an app started with `NO_AT_BRIDGE=1`, Chromium without its accessibility flags) now returns after 5 s instead of waiting the whole `--timeout` (30 s by default); `--activate` still waits until `--timeout`. The result is still `ok` with the spawned pid and no `window`, and its suggestion says the process did not join the bus. ([#81](https://github.com/gshost1/agent-desktop/pull/81))
* On macOS, give the Automation permission probe a 3 s limit. While a consent prompt waits on screen the probe blocked until someone answered it, 175 s in one run, and every command hung with it. Command preflight and the FFI `ad_check_permissions` no longer probe Automation at all; `status` and `permissions` report it as unknown when the probe does not answer, and a notification command that needs it fails with `APP_UNRESPONSIVE` and says a prompt may be waiting. ([#7](https://github.com/gshost1/agent-desktop/pull/7))
* On macOS, verify a headless `click` that opens a file or folder item through `AXOpen` by its effect: the sheet that held the item closes, or the window takes the item's name as its title. Such an open used to be reported as failed when the app answered with an uninformative code or reported the removed target as invalid. ([#9](https://github.com/gshost1/agent-desktop/pull/9))
* A session trace event waits for the shared budget lock as long as its command's deadline allows (up to 2 s, and up to 250 ms from the event when less is left) instead of a fixed 50 ms, and the recording ledger is rewritten in place without a disk sync per line. Eight parallel `--trace-strict` writers in one Linux session failed 128 of 200 commands on lock timeouts and now fail none, and a traced command costs 7.4 ms instead of 21.2 ms. ([#83](https://github.com/gshost1/agent-desktop/pull/83))
* On macOS, `launch` checks on every poll whether the launched app is still running. An app that finishes starting up and then quits, with or without `--activate`, now fails with `APP_UNRESPONSIVE` "Launched application exited before it presented a window" and `details.pid` instead of answering `ok` with the pid of the exited process, which with `--activate` took the whole `--timeout`. An app that restores several windows with none focused now launches with no `data.window` and a `data.suggestion` that lists the window ids, instead of failing with `AMBIGUOUS_TARGET`. ([#88](https://github.com/gshost1/agent-desktop/pull/88))
* On Linux, `launch "<Name>"` where several desktop entries share the name and run the same command (one canonical executable reached under the same name, the same arguments and working directory, as when a package installs an app under two desktop ids such as `com.google.Chrome` and `google-chrome`) now starts the first of them in desktop id order that can start, instead of failing with `AMBIGUOUS_TARGET`. Entries that run different commands still fail with `AMBIGUOUS_TARGET`, now with `details.candidates` (each desktop id with its command or the reason it cannot start) and a suggestion to rerun the launch with a startable desktop id, instead of a suggestion to refresh refs. A running process now answers `--app` to every desktop entry that runs its exact command, and `list-apps` reports the first such desktop id as `bundle_id`. ([#89](https://github.com/gshost1/agent-desktop/pull/89))
* On Linux, a process that `launch` started from a desktop entry now answers `--app` to that entry's name and desktop id, and `list-apps` reports that id as `bundle_id`, even when the entry's command is a wrapper that `exec`s another binary or the launch passed arguments. After `launch google-chrome --cdp`, `--app "Google Chrome"` used to fail with `APP_NOT_FOUND` because the browser runs `/opt/google/chrome/chrome`, which no entry names. The launch records the entry for that exact process instance and executable in a private file under `$XDG_RUNTIME_DIR`. ([#PRNUM](https://github.com/gshost1/agent-desktop/pull/PRNUM))

### Performance

* On macOS, take the capture that confirms an app or window inventory right after the first one instead of 5 ms later. The pause now follows only a capture that changed or failed. `list-windows` fell 3-7 ms and `press` 3-16 ms (medians on five apps). ([#98](https://github.com/gshost1/agent-desktop/pull/98))
* On macOS, open the window-server connection, the accessibility connection to the target app and the two permission checks on background threads instead of one after another, for commands that read the desktop. Every command is a fresh process and paid 25-40 ms for each connection before it read anything. Medians on a warm app (Calculator, the SwiftUI fixture, Script Editor, System Settings, Weather) fell 25-38 ms for `list-windows`, 26-40 ms for `snapshot -i`, 28-42 ms for `find`, 19-37 ms for `press` and 9-13 ms for `get`, `set-value` and `click`. ([#95](https://github.com/gshost1/agent-desktop/pull/95))
* On macOS, count the children of an element that has none once instead of twice. The second count only proves a list did not change while it was read, and a childless element has no list to read. Snapshot, `find` and the settled-surface walk after a click make one accessibility round trip fewer per leaf: medians fell 10-17 ms on the SwiftUI fixture and Weather and 2-4 ms on smaller trees, with identical trees. ([#97](https://github.com/gshost1/agent-desktop/pull/97))
* On macOS, wait the 2 s accessibility settle only for a Chromium process, recognized by a bundled framework that ships the V8 startup snapshot or is named `Electron Framework` or `Chromium Embedded Framework`. The first snapshot of a freshly launched native app no longer pays it, and `launch` now reports `renderer: "chromium"` for browsers and renamed Electron builds too. ([#5](https://github.com/gshost1/agent-desktop/pull/5))

### Documentation

* Add a synopsis of `press`, `find`, `type`, `select` and `set-value` to the interaction reference, loaded on demand, with tests that keep each form in step with the command's flags. Taken from [#48](https://github.com/gshost1/agent-desktop/pull/48) without its additions to the core skill, which in a paired run made the three bench tasks use 14% more input tokens. ([#82](https://github.com/gshost1/agent-desktop/pull/82))
* Tell agents to take a path the task names, and the answer, in one call: `find` each step by name, `click` it with `-w` naming the next step, and end with a `grep`-filtered read, never `head`; `jq -e` stops the chain at the first miss. On the three macOS bench tasks (Opus 5.5, 8 runs per task per build, paired against main cce8fa1c) System Settings fell from 4 to 2 turns, 51.1K to 25.8K input tokens and 19.2 s to 12.4 s (medians); the summed medians went from 48.6 s to 43.2 s and 119.0K to 95.4K tokens, 24 of 24 passing on both. ([#85](https://github.com/gshost1/agent-desktop/pull/85))

## [1.3.0](https://github.com/gshost1/agent-desktop/compare/v1.2.0...v1.3.0) (2026-11-09)

This fork release publishes GitHub release archives and checksums only. It does not publish to npm.

### Features

* Drive web pages in Chrome through the snapshot and ref loop over the Chrome DevTools Protocol (`--cdp`). A tab is read and acted on in the background, without raising the window or moving the cursor. ([#28](https://github.com/gshost1/agent-desktop/pull/28))
* Add `wait --download`, which returns the saved path of a browser download. Launch the browser with `--cdp --download-dir <dir>` to follow a download an earlier command started. ([#47](https://github.com/gshost1/agent-desktop/pull/47))
* On macOS, run a `cmd` chord given to `press --app` through the app's menu item when one owns the chord, so it works while the app is in the background. ([#25](https://github.com/gshost1/agent-desktop/pull/25))
* Add `launch --reopen` to opt in to the Launch Services reopen that `launch --activate` no longer sends to a running app. ([#54](https://github.com/gshost1/agent-desktop/pull/54))
* Add `--lines`, an opt-in global flag that prints a snapshot tree as `data.lines`, one indented line per node with its ref first, then role, quoted name and value. It replaces `data.tree` only when set; default output is unchanged and the envelope version stays 2.6. It applies to `snapshot`, `--root` drills, the snapshot `-w` returns, and batch entries, and `mcp --lines` turns it on for an MCP server. The agent skill points agents at it. This is an additive contract change; `docs/json-output.md` has the grammar under "Snapshot lines". ([#74](https://github.com/gshost1/agent-desktop/pull/74))

### Bug Fixes

* On macOS, `launch --activate` on a running app brings it forward without opening a window nobody asked for. `data.launch_path` reports how the app was reached. ([#54](https://github.com/gshost1/agent-desktop/pull/54))
* On macOS, send `press` keys to the process that serves an Open or Save panel, so Go to Folder, Return and typed keys reach the panel. ([#26](https://github.com/gshost1/agent-desktop/pull/26))
* On macOS, judge Return and Escape on a sheet or dialog by the effect on the surface, and let a sheet or popover frame replace its window's viewport. ([#50](https://github.com/gshost1/agent-desktop/pull/50)) ([#27](https://github.com/gshost1/agent-desktop/pull/27))
* On macOS, write text into the in-place editor of a renamed list row and leave the commit pending, instead of reporting a write that Return then discards. ([#24](https://github.com/gshost1/agent-desktop/pull/24))
* On macOS, name file-list rows from their single text field so `find --name` matches them in a Save panel. ([#56](https://github.com/gshost1/agent-desktop/pull/56))
* On macOS, wait for a row's own selection to settle before verifying it, so a `click` on a list or outline cell that did select the row no longer fails now and then with `ACTION_FAILED` and `delivered_unverified`. ([#76](https://github.com/gshost1/agent-desktop/pull/76))
* Give `close-app` a budget per phase, with a new `--timeout-ms` (default 5000), so waiting for another command's desktop lease can no longer use up the exit check. A close that was sent and whose app then exits now reports `delivered_verified` or `delivered_unverified` instead of `TIMEOUT` or, on macOS, `APP_UNRESPONSIVE`; a `TIMEOUT` names its phase in `details.phase`. ([#38](https://github.com/gshost1/agent-desktop/pull/38))
* When a browser that `launch --cdp` started lists no window, `snapshot`, `find` and `wait` with `--surface web`, `open-url` and saved web refs now read the page through that browser process instead of failing with `WINDOW_NOT_FOUND`. Without an endpoint recorded for that exact process, the result is still `WINDOW_NOT_FOUND`. The window was most likely off the current Space; that cause is unconfirmed. ([#77](https://github.com/gshost1/agent-desktop/pull/77))
* On macOS, a headless `right-click` whose `AXShowMenu` times out while the menu it opened is tracking (`kAXErrorCannotComplete`) now reads the app's menu surface and reports `delivered_verified` when the menu is there, instead of `APP_UNRESPONSIVE`. An app that answers but shows no menu gets `ACTION_FAILED` with `details.menu_observed: false`; an app that never answers keeps `APP_UNRESPONSIVE`. ([#36](https://github.com/gshost1/agent-desktop/pull/36))
* On macOS, when a headless `right-click` chain ends unverified, read whether a new pop-up menu window (window level 101) of the target's process appeared after a baseline that showed none, and report `delivered_verified` when it did. This covers context menus the accessibility tree does not expose, such as a SwiftUI text field's, which returned `ACTION_FAILED` although the menu opened. Each right-click now costs about 115 ms more for the baseline read (median 860 → 973 ms on a test text field, 527 → 643 ms on a Font Book sidebar row). ([#79](https://github.com/gshost1/agent-desktop/pull/79))
* On macOS, find the window of a control that omits `AXWindow`, so `select`, `click` and `expand` on System Settings popups no longer time out. ([#6](https://github.com/gshost1/agent-desktop/pull/6))
* On macOS, read window titles through accessibility without Screen Recording permission, so `list-windows`, `snapshot` and `wait --window` can tell windows apart. ([#32](https://github.com/gshost1/agent-desktop/pull/32))
* Stop `find` and surface walks from repeating reads that cannot finish within the budget. ([#33](https://github.com/gshost1/agent-desktop/pull/33))
* Keep `find` and snapshots from reporting an incomplete walk with no cause. A list's rows read after the rest of the window could be skipped as false cycles, so `find` failed with `TIMEOUT` `locator_incomplete` in under a second and snapshots came back incomplete. ([#67](https://github.com/gshost1/agent-desktop/pull/67))
* Bound the surface report after an action to one second, so a slow report no longer adds its full timeout to the action. ([#4](https://github.com/gshost1/agent-desktop/pull/4))
* Give a batch's permission preflight its own budget, capped at 15 seconds, so a slow check no longer fails the batch with `TIMEOUT` before its first entry. ([#57](https://github.com/gshost1/agent-desktop/pull/57))
* Keep a paced `type` burst from reporting delivered text as unsent when the refmap changes mid-burst. ([#2](https://github.com/gshost1/agent-desktop/pull/2))
* Mark every pre-dispatch ref-action refusal `not_delivered`, and settle a failed command's delivery by what the command can change. ([#10](https://github.com/gshost1/agent-desktop/pull/10)) ([#21](https://github.com/gshost1/agent-desktop/pull/21))
* Report the follow-up read after `focus-window` as data, and trust the adapter's focus confirmation instead of failing a confirmed focus. ([#46](https://github.com/gshost1/agent-desktop/pull/46))
* Write `session start --cursor` in one manifest write, so a failed start leaves no hidden session. ([#35](https://github.com/gshost1/agent-desktop/pull/35))
* Pair each trace `command.end` with its own `command.start`, so overlapping commands on one session no longer report false unpaired events. ([#51](https://github.com/gshost1/agent-desktop/pull/51))
* Make the MCP server forward only server-wide global flags and refuse per-command flags at startup, and lend an inherited interaction lease to one input-capable call at a time. ([#52](https://github.com/gshost1/agent-desktop/pull/52)) ([#13](https://github.com/gshost1/agent-desktop/pull/13))
* Hold the session lock across manifest updates, write the trace usage ledger atomically, and keep a cancelled MCP call id occupied until its call finishes. ([#1](https://github.com/gshost1/agent-desktop/pull/1))
* In the C library, return each failing call's own result code, and let a log callback call back into the library. ([#37](https://github.com/gshost1/agent-desktop/pull/37)) ([#55](https://github.com/gshost1/agent-desktop/pull/55))
* Tell agents to brace a snapshot-qualified ref built from a shell variable (`"${S}:e1"`). In zsh, an unbraced `$S:e1` drops the snapshot id and passes `1`. ([#63](https://github.com/gshost1/agent-desktop/pull/63))
* On the web channel, give native date, time, datetime-local, month, week and color inputs one ref and set them whole with `set-value`, reported `delivered_verified` only when the read-back matches. A value in another format is refused with the exact value to pass, a part of the input is refused with the input it belongs to, and a batch now reports `failed_entries`. ([#66](https://github.com/gshost1/agent-desktop/pull/66))
* Return a full snapshot whose refs would pass the 1 MB refmap cap, such as a long web page, as a truncated tree (`complete: false`, `refmap_capped: true`) instead of failing with `INTERNAL`. `wait --text` and `--wait-for` report `match_cut` when the match lies past the cut. A `--root` drill past the cap still fails with `INTERNAL`. ([#64](https://github.com/gshost1/agent-desktop/pull/64))
* Answer `STALE_REF` (`details.process_state: exited`, `retryable: false`) instead of `APP_NOT_FOUND` for a ref whose application exited or crashed, on every platform, and for `snapshot --root` drills of such a ref. This changes the error code callers see: code that relaunches on `APP_NOT_FOUND` for a ref action must now check `STALE_REF` with `details.process_state`. A process that still runs, is unresponsive, or whose state cannot be read keeps `APP_NOT_FOUND`. ([#69](https://github.com/gshost1/agent-desktop/pull/69))
* On the web channel, give a ref to a row that a page exposes only as plain text when the row has its own click listener. ([#65](https://github.com/gshost1/agent-desktop/pull/65))
* Keep a command's permission preflight inside its own timeout plus one second: with no inherited deadline it gets the command's timeout plus 900 ms, at most 5 s, so a silent session bus or display no longer stretches `snapshot --timeout-ms 1000` to 5 s. On Linux the display capture probe drops from 1 s to 500 ms. ([#72](https://github.com/gshost1/agent-desktop/pull/72))
* On Linux, answer `APP_UNRESPONSIVE` at once when a ref action's auto-wait finds the app's process stopped, instead of reading it until `TIMEOUT`. ([#73](https://github.com/gshost1/agent-desktop/pull/73))
* On Linux, confirm a window placement that the window manager applies late, report an X11 emergency release only when a key or button was held, and give back a pre-maximize place only while the window is as the agent left it. ([#43](https://github.com/gshost1/agent-desktop/pull/43)) ([#15](https://github.com/gshost1/agent-desktop/pull/15)) ([#22](https://github.com/gshost1/agent-desktop/pull/22))

### Performance

* A web snapshot now reads each block's inline text as one name instead of one node per fragment: on a long article (saved Wikipedia page) the snapshot JSON is 39% smaller (1,091,020 → 663,888 bytes, 12,273 → 4,508 nodes), and `-i` output is 17% smaller, with refs unchanged and no snapshot latency cost (alternating binaries on one Chrome, n=25: within ±2%). Limit: a syntax-highlighted `pre` block reads as one text node with its newlines and indentation collapsed to spaces. ([#78](https://github.com/gshost1/agent-desktop/pull/78))
* On four local test pages, four web tasks through `--cdp` took 32.8 s and 138.4K input tokens. The benchmark ran on the `ad-web` browser channel (Sonnet, headed, 3 runs per task, sum of the medians, 15 of 15 runs passed). The same tasks through the accessibility tree alone, on an earlier build, took 166 s and 952K tokens, and through agent-browser, on an earlier build, 47 s and 171K tokens. A task from a local test page to Calculator took 17.7 s and 90.5K tokens. On five tasks against real sites (a long Wikipedia article, GitHub release and star count, a weather page, a multi-field form, a docs lookup), built from main d48657e9 (agent-desktop 1.2.0 + #64–#66, #77, #78), n=3 per task, Sonnet drivers, headed, rotated order, Chrome quit gracefully each run: agent-desktop's web channel passed 14/15 in 228 s total (mean 15.2 s, 103 turns, 1.23M input tokens) against agent-browser 0.41.2 over CDP at 15/15 in 287 s (mean 19.2 s, 119 turns, 0.94M input tokens), so it is 21% faster in total time and uses 30% more input tokens; the one miss is a correct answer padded past the checker's length cap. Before the fixes (a8b03477, same tasks) the same channel passed 13/15 in 831 s with 3.88M input tokens. With 3 runs per task, model latency adds noise, and the local-page accessibility-only and agent-browser figures were not rerun.
* On three macOS bench tasks (Weather, System Settings, Calculator; Opus 5.5; 8 runs per task per build, order rotated, one paired session), main cf03b9d3 against upstream b43ac4f6: the summed median time went from 52.0 s to 43.2 s and the summed mean from 53.4 s to 43.0 s; summed median input tokens fell from 174K to 118K and median turns from 12 to 9; 24 of 24 runs passed on both builds. Weather fell from 18.6 s to 13.3 s, System Settings from 18.8 s to 16.4 s and Calculator from 14.5 s to 13.5 s (medians). Three skill and output changes carry it: one-call forms that act and read back in a single command ([#70](https://github.com/gshost1/agent-desktop/pull/70)), `--lines` for a compact tree ([#74](https://github.com/gshost1/agent-desktop/pull/74)), and `e`-prefixed loop items for refs built from a variable ([#75](https://github.com/gshost1/agent-desktop/pull/75)), which took Calculator's mistyped first attempts from 3 of 8 on upstream to 0 of 8. Absolute times drift with machine load between runs of the same build (main summed 42.1 s at 1534ccf9 and 57.9 s at c2e24adb an hour later, with no macOS change between them; upstream summed 45.9 s earlier in the evening and 52.0 s in this run), so only the paired comparison carries meaning. Caveats: 8 runs per task, and model latency varies between runs.

### Documentation

* Tell agents that a save panel's name field takes a file name, not a path. ([#3](https://github.com/gshost1/agent-desktop/pull/3))
* Tell agents to take the snapshot right after a web click instead of sleeping. ([#60](https://github.com/gshost1/agent-desktop/pull/60))

### Testing

* The macOS E2E suite ran against main 15aa75cb on the release Mac with private steps skipped (`AGENT_DESKTOP_E2E_SKIP_PRIVATE=1`): 122 passed, 9 failed, 2 skipped, and 3 private steps skipped. Five of the failures (headed click, headed drag, the wait-actionable click, trace screenshots with Screen Recording denied, and a removed row refused with `APP_UNRESPONSIVE`) also failed in an earlier run on the same Mac on 2026-11-07 ([#32](https://github.com/gshost1/agent-desktop/pull/32)).

### Known issues

* A web snapshot reads a block's inline text as one name, so `find --native-id` can no longer reach the id of a non-interactive inline span inside it, and two small test pages grew 3–5% in `-i` bytes. ([#78](https://github.com/gshost1/agent-desktop/pull/78))
* On the web channel, a clickable row that a page exposes only as plain text still gets no ref when its click is handled by a listener on an ancestor (event delegation) or only on `pointerdown`.
* A ref to a removed row is refused with `APP_UNRESPONSIVE` instead of `STALE_REF`. Nothing is delivered (`not_delivered`); take a fresh snapshot before retrying.

## [1.2.0](https://github.com/vercel-labs/agent-desktop/compare/v1.1.0...v1.2.0) (2026-11-05)

### Features

* Add `agent-desktop mcp`, a Model Context Protocol server over stdio that exposes each leaf CLI command as a tool. Tool names and argument schemas come from the CLI definitions, and calls return the command's JSON envelope. ([#519](https://github.com/vercel-labs/agent-desktop/pull/519))
* Accept `--stdin` for `type`, `set-value`, and `clipboard-set` to read UTF-8 text from a pipe or redirected file instead of a command-line argument. ([#373](https://github.com/vercel-labs/agent-desktop/pull/373))
* On macOS, expose and select list rows whose selection can only be changed through their container. ([#503](https://github.com/vercel-labs/agent-desktop/pull/503))

### Bug Fixes

* Preserve partial snapshots and matched wait results when the observation deadline expires instead of failing while saving their refs. ([#372](https://github.com/vercel-labs/agent-desktop/pull/372))
* On macOS, ignore exited apps in the workspace inventory instead of failing app discovery. ([#371](https://github.com/vercel-labs/agent-desktop/pull/371))
* On macOS, draw the cursor overlay above menus and system panels. ([#511](https://github.com/vercel-labs/agent-desktop/pull/511))
* On macOS, start the cursor overlay outside the caller's working directory so the overlay does not keep that directory in use. ([#381](https://github.com/vercel-labs/agent-desktop/pull/381))

### Documentation

* Document installation from Homebrew's default catalog with `brew install agent-desktop`. ([#524](https://github.com/vercel-labs/agent-desktop/pull/524))

## [1.1.0](https://github.com/vercel-labs/agent-desktop/compare/v1.0.2...v1.1.0) (2026-11-02)

### ⚠ BREAKING CHANGES

* The C ABI major version is now 7. `ad_resolve_element_exact` and `ad_execute_ref_action_exact_with_policy` always refuse with `ACTION_NOT_SUPPORTED` (`details.kind: exact_entry_unsupported`): a C caller cannot supply the capture proof a ref now resolves by, so act through a snapshot ref instead. ([#442](https://github.com/vercel-labs/agent-desktop/pull/442)) by @lahfir
* The response envelope moves to 2.6. On Windows and Linux, a ref to one of several identical controls, a ref from a window the snapshot could not read whole, and a ref from an earlier snapshot now return `AMBIGUOUS_TARGET` with `details.kind: identity_not_unique` where some used to act. A snapshot that could not read its window whole to prove its refs now says so with `data.identity_unproven: true`. See the version 2.6 migration in `docs/json-output.md`. ([#442](https://github.com/vercel-labs/agent-desktop/pull/442)) by @lahfir
* A surface kind the OS does not have now returns `INVALID_ARGS` instead of `PLATFORM_NOT_SUPPORTED`, and names the kinds to use. FFI callers see the matching `AdResult` code. An adapter that has no snapshot surfaces at all still returns `PLATFORM_NOT_SUPPORTED`. ([#480](https://github.com/vercel-labs/agent-desktop/pull/480)) by @lahfir

### Features

* Reach the macOS Dock and menu bar status items with `snapshot --surface dock` and `--surface menu-bar-extras`. With `--app`, `menu-bar-extras` reads that app's own status item. `open-system-surface` returns either surface at once, headless, with no `window` in its data. ([#480](https://github.com/vercel-labs/agent-desktop/pull/480)) by @lahfir
* Observe a macOS background app that has no window, such as a menu bar app, the Dock, or the Control Center and Notification Center panels, through its process. Its snapshot leaves out `data.window`. `--app` also matches an app's executable name or bundle ID, so `--app ControlCenter` works. ([#480](https://github.com/vercel-labs/agent-desktop/pull/480)) by @lahfir
* Add `type --interval-ms` to type one keystroke every N ms (1 to 2000), verified against the field after the last keystroke. ([#348](https://github.com/vercel-labs/agent-desktop/pull/348)) by @ctate
* Show the insertion caret in the cursor overlay after `type` and `set-value`, on macOS and Windows. ([#349](https://github.com/vercel-labs/agent-desktop/pull/349), [#350](https://github.com/vercel-labs/agent-desktop/pull/350)) by @ctate
* Draw the cursor label as a black pill sized to its text, on macOS and Windows. ([#357](https://github.com/vercel-labs/agent-desktop/pull/357)) by @ctate
* Colour the cursor body with the accent by default. `--fill` sets a different body colour. ([#361](https://github.com/vercel-labs/agent-desktop/pull/361)) by @ctate
* Draw `--image` and `--pointer-image` cursors on Windows. ([#345](https://github.com/vercel-labs/agent-desktop/pull/345)) by @ctate
* Start a headless action's cursor travel before it takes the interaction lease, so the cursors of parallel agents move at the same time. ([#347](https://github.com/vercel-labs/agent-desktop/pull/347)) by @ctate

### Bug Fixes

* Remove the macOS command host. Every CLI call runs in its own process, as on Windows and Linux. The npm install and every CLI call stop any command host an earlier version left running. After an install that skips npm scripts, or a manual install, run `agent-desktop version` once. ([#414](https://github.com/vercel-labs/agent-desktop/pull/414)) by @lahfir
* Refs from snapshots taken before this release carry no capture proof, so take a new snapshot. On Windows and Linux they return `AMBIGUOUS_TARGET` with `details.kind: identity_not_unique`; on macOS a ref still resolves while the element its token names is alive. ([#442](https://github.com/vercel-labs/agent-desktop/pull/442)) by @lahfir
* A ref does not act on a different element after the UI changes. On macOS a ref follows its own element: a ref to a removed row returns `STALE_REF`, and identical controls can each be used by ref. On Windows and Linux a ref resolves only if it was unique when captured and exactly one element matches it now; a ref to one of several identical controls returns `AMBIGUOUS_TARGET`. Identity is the element's role and stable fields, not its instance: an identical element that replaced the original is acted on, and a toolkit that keeps a removed widget listed (GTK4, Qt6 on Linux) can still receive the action. ([#441](https://github.com/vercel-labs/agent-desktop/pull/441), [#442](https://github.com/vercel-labs/agent-desktop/pull/442)) by @lahfir
* On macOS, a `--headed` click on a status item or on a background app's control sends a real click, without first focusing a window the app does not have. ([#480](https://github.com/vercel-labs/agent-desktop/pull/480)) by @lahfir
* On macOS, a press whose return code proves nothing counts as delivered when focus moves, or the focused element changes, inside the pressed element's window. Notes' New Note no longer reports `ACTION_FAILED` after creating the note. ([#480](https://github.com/vercel-labs/agent-desktop/pull/480)) by @lahfir
* On macOS, `set-value` on a stepped control re-reads the value after each step before giving up, and says when the steps cannot reach the target. ([#480](https://github.com/vercel-labs/agent-desktop/pull/480)) by @lahfir
* Keep the overlay caret on the insertion point after key presses. On macOS, place it on an indented new line. ([#353](https://github.com/vercel-labs/agent-desktop/pull/353), [#354](https://github.com/vercel-labs/agent-desktop/pull/354)) by @ctate
* On macOS, draw overlay controls as they arrive, hide a cleared label, and stop clipping the click highlight's glow. ([#346](https://github.com/vercel-labs/agent-desktop/pull/346), [#352](https://github.com/vercel-labs/agent-desktop/pull/352)) by @ctate
* On Windows, stop every command ending in a crash report and about 3 s of error reporting at exit. ([#342](https://github.com/vercel-labs/agent-desktop/pull/342)) by @ctate
* On Windows, type CR LF as one line break. ([#368](https://github.com/vercel-labs/agent-desktop/pull/368)) by @Railly
* On Windows, stop scaling the overlay's coordinates twice, which sent the cursor off screen at 200% scaling, and stop the overlay flashing a black square when it starts. ([#343](https://github.com/vercel-labs/agent-desktop/pull/343), [#344](https://github.com/vercel-labs/agent-desktop/pull/344)) by @ctate

## [1.0.2](https://github.com/vercel-labs/agent-desktop/compare/v1.0.1...v1.0.2) (2026-10-30)

### Features

* Tell npm users when the Node launcher slows each call. When npm blocks install scripts, the launcher prints a one-time notice to stderr with the command that links the native binary.

### Bug Fixes

* Make `permissions --guide` on macOS offer the app that launched agent-desktop, such as a terminal that LaunchServices does not know, instead of the agent-desktop binary.
* Restart the command host after it reports a missing permission, so commands succeed after the user grants access.
* Point the permission hints and skills at `permissions --guide` instead of `--request`.
* Point the npm package and repository links at vercel-labs/agent-desktop.

## [1.0.1](https://github.com/vercel-labs/agent-desktop/compare/v1.0.0...v1.0.1) (2026-10-27)

### Bug Fixes

* Remove the `version` line from the skills. In 1.0.0 it still said 0.11.1; `agent-desktop version` reports the version.
* Publish npm and GitHub releases automatically when the Cargo workspace version changes on main.
* Remove the custom Homebrew tap, the Scoop bucket, and their package-update pull requests.

## [1.0.0](https://github.com/vercel-labs/agent-desktop/compare/v0.11.1...v1.0.0) (2026-10-27)

### Release

* Add Linux support: the CLI, the C library, and the npm package now run on x86_64 and aarch64 Linux (glibc 2.35 or later) through AT-SPI.
* Release the integrated macOS, Windows, and Linux CLI and C library as 1.0.0.
* Bundle native CLI executables and platform helpers in npm installations, including installations with lifecycle scripts disabled.
* Verify release archives, checksums, and build provenance through the manual release workflow.

## [0.10.1](https://github.com/vercel-labs/agent-desktop/compare/v0.10.0...v0.10.1) (2026-10-18)


### Bug Fixes

* **windows:** let launch take a bare name without .exe ([#276](https://github.com/vercel-labs/agent-desktop/issues/276)) ([483427e](https://github.com/vercel-labs/agent-desktop/commit/483427eb84821a9d0ebc79eb56dd689c04cb15be))

## [0.10.0](https://github.com/vercel-labs/agent-desktop/compare/v0.9.4...v0.10.0) (2026-10-18)


### ⚠ BREAKING CHANGES

* the release carries these contract changes.
    - The C ABI major version is 5. The legacy entrypoints that each have an
      *_exact replacement are removed: ad_list_surfaces, ad_surface_list_count,
      ad_surface_list_get, ad_surface_list_free, ad_list_windows,
      ad_window_list_count, ad_window_list_get, ad_window_list_free,
      ad_launch_app, ad_release_window_fields, ad_get_tree, ad_find, ad_is,
      ad_focus_window, ad_window_op, ad_resolve_element and
      ad_execute_ref_action_with_policy, with the AdSurfaceList and AdWindowList
      handles. AdScreenshotKind loses WINDOW (FULL_SCREEN keeps 2 and 1 is
      rejected); AdScreenshotTarget loses pid; AdExactSurfaceInfo is version 2,
      48 bytes, with a trailing unclassified field; the snapshot_id argument of
      ad_execute_by_ref and ad_execute_by_ref_timeout and the
      AdWaitPredicate.snapshot_id field are removed, so AdWaitPredicate is 40
      bytes and AdWaitArgs 104. Consumers must rebuild against the regenerated
      header.
    - Bare @eN ref input is removed from every ref-taking CLI command, wait
      --element, snapshot --root, find --root, drag --from/--to, batch entries
      and the FFI. Only the snapshot-qualified @<snapshot_id>:eN is accepted, and
      a bare ref fails with INVALID_ARGS. The --snapshot flag that only qualified
      a bare ref is removed from every command, and batch entries carrying a
      snapshot or snapshot_id field are rejected as unknown fields.
    - get --property text no longer returns the same value as --property value.
      It prefers the value for textfield, combobox, listbox, datefield and
      timefield and falls back to the accessible name; every other role prefers
      the accessible name and falls back to the value. Blank strings count as
      absent. Ask for --property value explicitly to keep the previous answer.
    - An io error whose kind is PermissionDenied answers PERM_DENIED instead of
      INTERNAL, in the CLI envelope and in the FFI envelope, return code and
      last error alike.
    - A ref action whose element is gone no longer ends in TIMEOUT. A
      non-retryable resolution failure answers at once; a retryable one keeps
      polling, and if no later resolution succeeds the action answers the
      recorded STALE_REF or AMBIGUOUS_TARGET at the deadline.

### Features

* support Windows 10 1809+ and Windows Server 2019+ on x64 and ARM64 ([483a31d](https://github.com/vercel-labs/agent-desktop/commit/483a31d036175f54c34ce1fbd2cea762560ab78c))

## [0.9.4](https://github.com/vercel-labs/agent-desktop/compare/v0.9.3...v0.9.4) (2026-09-23)


### Bug Fixes

* sync cargo lock with the 0.9.3 release ([#217](https://github.com/vercel-labs/agent-desktop/issues/217)) ([8c1502e](https://github.com/vercel-labs/agent-desktop/commit/8c1502e43c6c78fe856980f0bb7b3585f877d347))

## [0.9.3](https://github.com/vercel-labs/agent-desktop/compare/v0.9.2...v0.9.3) (2026-09-22)


### Bug Fixes

* harden native accessibility targeting and delivery ([#210](https://github.com/vercel-labs/agent-desktop/issues/210)) ([882c4c4](https://github.com/vercel-labs/agent-desktop/commit/882c4c4695c0db7d75b83f2655ece178591deb6c))

## [0.9.2](https://github.com/vercel-labs/agent-desktop/compare/v0.9.1...v0.9.2) (2026-09-17)


### Bug Fixes

* make headless desktop actions report what actually happened, and add jev-desktop ([#206](https://github.com/vercel-labs/agent-desktop/issues/206)) ([0691eba](https://github.com/vercel-labs/agent-desktop/commit/0691eba25f6a7082b197bc73ad5daa0e3315c976))

## [0.9.1](https://github.com/vercel-labs/agent-desktop/compare/v0.9.0...v0.9.1) (2026-09-13)


### Bug Fixes

* stop the inspector shutdown test failing on an expected reset ([22cad9e](https://github.com/vercel-labs/agent-desktop/commit/22cad9ec21136562ed191a8cfb827dc22378c554))
* **drag:** scope `--wait-for` to the drop target by default, with `--wait-for-scope from|to` ([cefc80a](https://github.com/vercel-labs/agent-desktop/commit/cefc80a89c95b21061573e366e3832bff6baa231))
* preserve `wait_timeout` envelope when an inherited deadline expires ([07913a7](https://github.com/vercel-labs/agent-desktop/commit/07913a7cdb85d79e1708063fd398c9f8571894e9))
* prevent `wait --count` false positives on incomplete snapshots ([319e9c6](https://github.com/vercel-labs/agent-desktop/commit/319e9c6c2e7c175fdec42d19b2f1614b9238a909))
* split acronym boundaries in trace key redaction ([3042e0c](https://github.com/vercel-labs/agent-desktop/commit/3042e0c7a823508fd5417584f0459d428832c8a0))
* **cursor:** gate `agent_id` stamping on multi-agent mode ([58612ea](https://github.com/vercel-labs/agent-desktop/commit/58612eac673a45634e00a651872da043297e68d3))
* skip ended-session batch entries before wait baseline pre-capture ([899ce27](https://github.com/vercel-labs/agent-desktop/commit/899ce2702a68716a7704b7403050dd7f54161208))
* restore cursor overlay lifecycle for keyboard and clipboard actions ([c827cf9](https://github.com/vercel-labs/agent-desktop/commit/c827cf96560629265ea34451a1a66fe839adab8f))
* clear remembered cursor landing when the overlay is hidden ([6adff7a](https://github.com/vercel-labs/agent-desktop/commit/6adff7af0f50344924f8e8874be72ded1b72a8b3))
* reject percent-encoded NUL in `file_url_to_path` ([de5df38](https://github.com/vercel-labs/agent-desktop/commit/de5df38ede3d45ed7e4d438e732724972351dbe4))

## [0.9.0](https://github.com/vercel-labs/agent-desktop/compare/v0.8.5...v0.9.0) (2026-09-12)


### ⚠ BREAKING CHANGES

* the response envelope moves to 2.4. Stateful ref actions can now return ACTION_FAILED for an observed postcondition contradiction, and ActionResult::from_execution takes only action and steps.

### Features

* verify stateful ref actions against fresh platform state ([9bc91bc](https://github.com/vercel-labs/agent-desktop/commit/9bc91bc0ab53139d85e5c10a0ae0b3c3b4bc18b9))


### Bug Fixes

* guard the debug viewer image sink and unpin a flaky test deadline ([fca24f2](https://github.com/vercel-labs/agent-desktop/commit/fca24f2b0259bac83ff17a490052e1f2a9a9496f))
* report unread expand state as unverified rather than failed ([a7131c2](https://github.com/vercel-labs/agent-desktop/commit/a7131c241cce06687b15c53386fd7787d8fbfd72))

## [0.8.5](https://github.com/vercel-labs/agent-desktop/compare/v0.8.4...v0.8.5) (2026-09-06)


### Features

* combine multi-agent cursors with core and macOS optimizations ([#171](https://github.com/vercel-labs/agent-desktop/issues/171)) ([922bb7d](https://github.com/vercel-labs/agent-desktop/commit/922bb7d5b59f2cc8cfaad9896c81fb8fed545bec))

## [0.8.4](https://github.com/vercel-labs/agent-desktop/compare/v0.8.3...v0.8.4) (2026-08-28)


### Features

* rebuild the agent cursor overlay and fix seen-but-unactionable elements ([#145](https://github.com/vercel-labs/agent-desktop/issues/145)) ([5ec2504](https://github.com/vercel-labs/agent-desktop/commit/5ec2504bec0e9face5a69ef25c58bb1273772f2f))

## [0.8.3](https://github.com/vercel-labs/agent-desktop/compare/v0.8.2...v0.8.3) (2026-08-22)


### Features

* harden native automation and add cursor overlay ([#137](https://github.com/vercel-labs/agent-desktop/issues/137)) ([25a0087](https://github.com/vercel-labs/agent-desktop/commit/25a00877726d324c0ee64d84c8d989a5808a66d6))

## [0.8.2](https://github.com/vercel-labs/agent-desktop/compare/v0.8.1...v0.8.2) (2026-08-20)


### Features

* relocate the state root with AGENT_DESKTOP_HOME ([#135](https://github.com/vercel-labs/agent-desktop/issues/135)) ([a336b01](https://github.com/vercel-labs/agent-desktop/commit/a336b01e728893f379918b40157ab25f1c41fa80))

## [0.8.1](https://github.com/vercel-labs/agent-desktop/compare/v0.8.0...v0.8.1) (2026-08-14)


### Features

* drive Chromium apps through a verified DevTools endpoint from launch --cdp ([#129](https://github.com/vercel-labs/agent-desktop/issues/129)) ([366d348](https://github.com/vercel-labs/agent-desktop/commit/366d34803992d0595f31b19c8347bf7b26f5f277))

## [0.8.0](https://github.com/vercel-labs/agent-desktop/compare/v0.7.0...v0.8.0) (2026-08-13)


### ⚠ BREAKING CHANGES

* the response envelope is version 2.3. launch returns { app, pid, process_instance, window? } instead of a bare window object, and an application that presents no window is ok:true with window omitted rather than WINDOW_NOT_FOUND. The C ABI is unchanged: it still writes one window and reports WINDOW_NOT_FOUND when there is none.

### Features

* decide macOS delivery by observation and stop launch waiting on an uncaused event ([#125](https://github.com/vercel-labs/agent-desktop/issues/125)) ([298f1ff](https://github.com/vercel-labs/agent-desktop/commit/298f1ff21530958d765adf3834cdadccd4816a7a))

## [0.7.0](https://github.com/vercel-labs/agent-desktop/compare/v0.6.0...v0.7.0) (2026-08-02)


### ⚠ BREAKING CHANGES

* ENVELOPE_VERSION is now 2.2. `data.complete` is present on every successful snapshot, and a snapshot that exhausts its budget returns `ok: true` with `complete: false` where it previously returned a TIMEOUT error. Callers that branched on TIMEOUT to detect an oversized tree must read `complete` instead.

### Bug Fixes

* return observed trees and stop demanding renderer activation from shallow walks ([#117](https://github.com/vercel-labs/agent-desktop/issues/117)) ([32175e4](https://github.com/vercel-labs/agent-desktop/commit/32175e44c553b350c90c311560ac4d341be71632))

## [0.6.0](https://github.com/vercel-labs/agent-desktop/compare/v0.5.0...v0.6.0) (2026-07-26)


### ⚠ BREAKING CHANGES

* private artifacts on Windows are no longer written through ACL-hardened handles. That hardening guarded refmap, trace, and session files on a platform where no command can produce them, and it had never executed. It returns in Phase 2.1, built on Windows against a CI lane that runs it, under the constraints recorded in `docs/solutions/best-practices/never-ship-platform-code-that-ci-cannot-execute.md`.

### Refactoring

* remove speculative Win32 private-file layer from core, add real Windows/Linux test lanes ([#106](https://github.com/vercel-labs/agent-desktop/issues/106)) ([8ad66b8](https://github.com/vercel-labs/agent-desktop/commit/8ad66b8f2115704eed56e59e2709c4eddf3cffac))
  * deleted 1,062 LOC of Windows-only `unsafe` Win32 file I/O from `agent-desktop-core` and dropped the `windows-sys` dependency; Windows now uses the same portable `std::fs` path as every other non-unix target
  * this is a structural fix, not a downgrade: `std::fs::OpenOptions` defaults `share_mode` to `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`, precisely the flag the deleted code omitted and the cause of its sharing-violation failures
  * `cargo test` now runs on windows-latest and ubuntu-latest, so every platform-conditional branch in core is executed on each PR instead of only compiled

### Bug Fixes

* the new platform lanes exposed four pre-existing defects that were unreachable while only macOS ran tests:
  * trace files were opened append-only while `trace.rs` locks that handle; Windows `LockFileEx` rejects append-only handles, so all trace writing failed with `Access is denied`
  * `refs_lock` tests placed lock files directly in `/tmp`, which is root-owned and world-writable, so the private-parent check correctly refused them
  * a trace test interpolated a raw path into a JSON string literal, so Windows paths produced invalid escapes and the event was silently dropped instead of skipped
  * the default `SystemOps::permission_report` returned `Denied`, so CLI preflight reported `PERM_DENIED` on Windows and Linux when the adapter is simply unimplemented; it now reports `Unknown` and surfaces the honest `PLATFORM_NOT_SUPPORTED`, including over the FFI
* the pinned toolchain gained `clippy` and `rustfmt`, which `profile = "minimal"` had omitted

## [0.5.0](https://github.com/vercel-labs/agent-desktop/compare/v0.4.7...v0.5.0) (2026-07-20)


### ⚠ BREAKING CHANGES

* default-on auto-wait changes the timing of every previously-untouched ref-action call (bounded 5000 ms default; `--timeout-ms 0` restores single-shot). `ENVELOPE_VERSION` is now `2.1` (adds the `APP_UNRESPONSIVE` code and process state in error details). FFI ABI major is `3` (append-only struct evolution; `wait --event` is intentionally not exposed over FFI). The legacy string clipboard API is removed in favor of typed content. `key-down`/`key-up` fail closed until daemon-owned held input exists. `close-app` verifies termination and the osascript fallback path is removed. `--text` matching is subtree containment: `find --text X --first` returns the outermost matching container.

### Features

* implement Playwright-grade foundation contract ([3f32272](https://github.com/vercel-labs/agent-desktop/commit/3f322728b44548d2e22f4ee6ef4e6853af4e4550))
  * default-on auto-wait: every ref action waits for actionability (visible, enabled, stable, unoccluded) before dispatching, under a bounded budget
  * live `find` locator plus a serializable `LocatorQuery`, and honest `is --property visible`
  * three-way `hit_test` occlusion gate and core `scroll_into_view` before element actions
  * `list-displays` and honest `--screen` with per-display scale factor; `native_id` identity spine; window-id-first resolution
  * `ProcessState` classification with the `APP_UNRESPONSIVE` code (envelope `2.1`); typed `ActionStep` delivery tier
  * `LaunchOptions` (`--arg`/`--env`/`--cwd`/`--no-attach`); baseline-diff desktop signals via `wait --event`
  * typed clipboard (`Text`/`Image`/`FileUrls`); mouse modifier chords and a `mouse-wheel` primitive
  * capability-supertrait `PlatformAdapter` split with `not_supported()` defaults, so Windows and Linux inherit the contract

## [0.4.7](https://github.com/vercel-labs/agent-desktop/compare/v0.4.6...v0.4.7) (2026-07-02)


### Features

* add trace viewer and replay artifacts ([e3e1872](https://github.com/vercel-labs/agent-desktop/commit/e3e1872ff3088f03f3e50e65ebc9e2b068a17b5f))

## [0.4.6](https://github.com/vercel-labs/agent-desktop/compare/v0.4.5...v0.4.6) (2026-07-02)


### Features

* make sessions the first-class trace container ([35fa914](https://github.com/vercel-labs/agent-desktop/commit/35fa914b52bb0dffb0e630f9d9085789c3f81542))

## [0.4.5](https://github.com/vercel-labs/agent-desktop/compare/v0.4.4...v0.4.5) (2026-06-30)


### Features

* add --wait-for selector polling flags ([#86](https://github.com/vercel-labs/agent-desktop/issues/86)) ([ce23278](https://github.com/vercel-labs/agent-desktop/commit/ce232787b50270d6f590e11bd2b16c7354de623d))

## [0.4.4](https://github.com/vercel-labs/agent-desktop/compare/v0.4.3...v0.4.4) (2026-06-29)


### Features

* **macos,core:** harden adapter and core foundation with caller-controllable guardrails ([#82](https://github.com/vercel-labs/agent-desktop/issues/82)) ([94ce6c5](https://github.com/vercel-labs/agent-desktop/commit/94ce6c551fff4ffd2afc44ab6a9f655b4486da11))

## [0.4.3](https://github.com/vercel-labs/agent-desktop/compare/v0.4.2...v0.4.3) (2026-06-28)


### Bug Fixes

* **macos:** harden retained_handle null guard against release-only CFRetain(null) ([#80](https://github.com/vercel-labs/agent-desktop/issues/80)) ([a708fa0](https://github.com/vercel-labs/agent-desktop/commit/a708fa03326a03dbc82acc7e41bd3ec262a05248))

## [0.4.2](https://github.com/vercel-labs/agent-desktop/compare/v0.4.1...v0.4.2) (2026-06-27)


### Features

* **ffi:** Phase B and C — Python smoke harness, parity gates, build.rs codegen ([#77](https://github.com/vercel-labs/agent-desktop/issues/77)) ([9023f33](https://github.com/vercel-labs/agent-desktop/commit/9023f331b3981a41414ea0f92e2e5120a212e357))

## [0.4.1](https://github.com/vercel-labs/agent-desktop/compare/v0.4.0...v0.4.1) (2026-06-26)


### Features

* complete FFI C-ABI surface (Phase A): load-time ABI handshake, session-scoped adapter, JSON-envelope command entrypoints (version, status, snapshot, wait, execute-by-ref), an optional tracing log callback, and a unified error-envelope contract ([#67](https://github.com/vercel-labs/agent-desktop/issues/67))

## [0.4.0](https://github.com/vercel-labs/agent-desktop/compare/v0.3.1...v0.4.0) (2026-06-24)


### ⚠ BREAKING CHANGES

* the version command no longer accepts --json; it always emits the standard JSON envelope.

### Refactoring

* over-engineering audit cleanup ([#64](https://github.com/vercel-labs/agent-desktop/issues/64)) ([dbb2be6](https://github.com/vercel-labs/agent-desktop/commit/dbb2be639ecc1f979031818259f587f951086b3c))

## [0.3.1](https://github.com/vercel-labs/agent-desktop/compare/v0.3.0...v0.3.1) (2026-06-21)


### Bug Fixes

* harden macos stale ref resolution ([#62](https://github.com/vercel-labs/agent-desktop/issues/62)) ([9f144c2](https://github.com/vercel-labs/agent-desktop/commit/9f144c2cafe3ba0ade479c1b87ecac6cd88adcef))

## [0.3.0](https://github.com/vercel-labs/agent-desktop/compare/v0.2.3...v0.3.0) (2026-06-20)

### ⚠ BREAKING CHANGES

* **ffi:** `AD_POLICY_KIND_PHYSICAL` is now `AD_POLICY_KIND_HEADED` (discriminant `2` unchanged).
* **ffi:** `AdRefEntry` and `AdDragParams` include additional reliability metadata; C ABI consumers must zero-initialize and validate with `AD_REF_ENTRY_SIZE`, `AD_DRAG_PARAMS_SIZE`, and the `ad_*_size()` accessors.
* `close-app` graceful responses now return `{ "method": "graceful", "requested": true }` instead of claiming `closed: true` before the app has exited.

### Features

* add Playwright-grade ref reliability with strict late resolution, session-scoped snapshots, deterministic stale/ambiguous target handling, and skeleton drill-down preservation ([#54](https://github.com/vercel-labs/agent-desktop/pull/54)).
* add Playwright-style headed/headless interaction policy: accessibility-first dispatch remains default, while `--headed` enables focused physical fallbacks only when needed ([#54](https://github.com/vercel-labs/agent-desktop/pull/54)).
* add retrying wait predicates, JSONL traces with secret redaction, richer actionability reports, and shared CLI/FFI execution through the same resolver/actionability/dispatch ladder ([#54](https://github.com/vercel-labs/agent-desktop/pull/54)).

### Bug Fixes

* harden macOS ref actions across focus/window raising, disclosure expansion, scroll, drag, numeric set-value, menu detection, stale-ref recovery, protected-process errors, and close-app confirmation ([#54](https://github.com/vercel-labs/agent-desktop/pull/54)).
* fail closed on unsafe fallback ambiguity and stale refs, including symlink-safe refstore/latest-snapshot reads and deadline-bound resolver work ([#54](https://github.com/vercel-labs/agent-desktop/pull/54)).

### Performance

* reduce repeated live actionability reads and add deadline-bound resolver behavior, with large-app snapshot and per-command CLI wall-clock gates in the E2E suite ([#54](https://github.com/vercel-labs/agent-desktop/pull/54)).

### Tests

* add real-app E2E coverage for headless/headed ref actions, sessions, snapshots, traces, waits, skeleton drill-down, menus, surfaces, drag, expand/collapse, and performance checks ([#54](https://github.com/vercel-labs/agent-desktop/pull/54)).

## [0.2.3](https://github.com/vercel-labs/agent-desktop/compare/v0.2.2...v0.2.3) (2026-06-06)


### Bug Fixes

* harden macos ax window fallback ([3b266fd](https://github.com/vercel-labs/agent-desktop/commit/3b266fdf040bf83438f69a400b032fd12b8715c6))
* resolve fullscreen AX tree retrieval returning ref_count: 0 ([a52b7c7](https://github.com/vercel-labs/agent-desktop/commit/a52b7c704a4d13fdc0d6b72f4080bb0fc118be64))

## [0.2.2](https://github.com/vercel-labs/agent-desktop/compare/v0.2.1...v0.2.2) (2026-06-02)


### Bug Fixes

* **macos:** guard CFArray casts with type-ID check (fixes Mail.app crash) ([#50](https://github.com/vercel-labs/agent-desktop/issues/50)) ([c02cb5e](https://github.com/vercel-labs/agent-desktop/commit/c02cb5ecb7314f053d63937437e5f5ba48de3209))

## [0.2.1](https://github.com/vercel-labs/agent-desktop/compare/v0.2.0...v0.2.1) (2026-05-23)


### Bug Fixes

* stabilize empty accessibility identity refs ([1fb5a7d](https://github.com/vercel-labs/agent-desktop/commit/1fb5a7d51eb798100b4d597c755fee1161e298bf))

## [0.2.0](https://github.com/vercel-labs/agent-desktop/compare/v0.1.14...v0.2.0) (2026-05-20)


### ⚠ BREAKING CHANGES

* chain execution deadlines now return TIMEOUT instead of ACTION_FAILED when the target app does not respond before the chain deadline.

### Refactoring

* unify command execution contracts ([1291a9c](https://github.com/vercel-labs/agent-desktop/commit/1291a9cdbf0566424d38da1eab397d6d4091c06c))

## [0.1.14](https://github.com/vercel-labs/agent-desktop/compare/v0.1.13...v0.1.14) (2026-05-04)


### Features

* bundle skill docs and refactor --help for AI agents ([#36](https://github.com/vercel-labs/agent-desktop/issues/36)) ([b04d6f9](https://github.com/vercel-labs/agent-desktop/commit/b04d6f97317af67648890d2d3b5ead0d27c466c9))

## [0.1.13](https://github.com/vercel-labs/agent-desktop/compare/v0.1.12...v0.1.13) (2026-04-17)


### Features

* **ffi:** ship C-ABI cdylib with review fixes and release pipeline ([#26](https://github.com/vercel-labs/agent-desktop/issues/26)) ([3cffbd6](https://github.com/vercel-labs/agent-desktop/commit/3cffbd67f6b27f42001643bef9fd2530cb7f9003))

## [0.1.12](https://github.com/vercel-labs/agent-desktop/compare/v0.1.11...v0.1.12) (2026-04-16)


### Features

* progressive skeleton traversal with ref-rooted drill-down ([#20](https://github.com/vercel-labs/agent-desktop/issues/20)) ([c17f2fa](https://github.com/vercel-labs/agent-desktop/commit/c17f2fae7abbbe2c914a050fa9e9be5fca9c6af0))

## [0.1.11](https://github.com/vercel-labs/agent-desktop/compare/v0.1.10...v0.1.11) (2026-03-03)


### Bug Fixes

* show skill install prompt on all success paths ([39b2bc6](https://github.com/vercel-labs/agent-desktop/commit/39b2bc63480890f7ed417b2c040eecf80c4628a0))

## [0.1.10](https://github.com/vercel-labs/agent-desktop/compare/v0.1.9...v0.1.10) (2026-03-03)


### Bug Fixes

* fix skill publication authentication in CI ([208af12](https://github.com/vercel-labs/agent-desktop/commit/208af12459fea2255e1c80b8cdc9ac420316d769))

## [0.1.9](https://github.com/vercel-labs/agent-desktop/compare/v0.1.8...v0.1.9) (2026-03-03)


### Features

* scalable skill architecture with publication automation ([#14](https://github.com/vercel-labs/agent-desktop/issues/14)) ([9766520](https://github.com/vercel-labs/agent-desktop/commit/97665203a464e605bc9b156ec90029c5909399be))

## [0.1.8](https://github.com/vercel-labs/agent-desktop/compare/v0.1.7...v0.1.8) (2026-03-01)


### Features

* add electron/web app compatibility for accessibility tree traversal ([a19c1b5](https://github.com/vercel-labs/agent-desktop/commit/a19c1b5132d3b71c5de58886ba51357ffc9bd1e8))
* implement --compact flag to collapse single-child unnamed nodes ([4a300c8](https://github.com/vercel-labs/agent-desktop/commit/4a300c8cb054462ca95fb5160e89e8fce661ec3b))

## [0.1.7](https://github.com/vercel-labs/agent-desktop/compare/v0.1.6...v0.1.7) (2026-02-28)


### Features

* add notification command types, adapter trait, and CLI wiring ([c5b05ba](https://github.com/vercel-labs/agent-desktop/commit/c5b05bab600aafa36c642f21837c44583b36459c))
* add notification management commands (macOS) ([b1fd368](https://github.com/vercel-labs/agent-desktop/commit/b1fd368f195640642adf011b75cca6ecb9e5acc3))
* **macos:** add NC session RAII guard and notification adapter wiring ([0d55c21](https://github.com/vercel-labs/agent-desktop/commit/0d55c21de0f0ea6ea08ccb836e5527c9da513620))
* **macos:** implement dismiss and notification action commands ([53d697d](https://github.com/vercel-labs/agent-desktop/commit/53d697d52248c3fa06797787b1eb549ac2766533))
* **macos:** implement notification list via AX tree traversal ([53549a3](https://github.com/vercel-labs/agent-desktop/commit/53549a384eec67572ee05c31621b8b4174425ab3))


### Bug Fixes

* **macos:** remove AXPress from dismiss action list ([27ef4f3](https://github.com/vercel-labs/agent-desktop/commit/27ef4f34c038c26f5852bf1f6026762a98d0df0a))
* **macos:** restore frontmost app after notification center interaction ([3881bc8](https://github.com/vercel-labs/agent-desktop/commit/3881bc82bdb5a4bb7689f1e1e2237bb745e60c21))
* **macos:** use pgrep and async osascript for NC lifecycle ([9797585](https://github.com/vercel-labs/agent-desktop/commit/979758538fca0145e9245641fb03a0769eed68de))

## [0.1.6](https://github.com/vercel-labs/agent-desktop/compare/v0.1.5...v0.1.6) (2026-02-24)


### Bug Fixes

* handle null bounds in refmap and improve sidebar click resolution ([d4197e8](https://github.com/vercel-labs/agent-desktop/commit/d4197e8f6f6700f2f672d3e1e436ecf24cf82e01))

## [0.1.5](https://github.com/vercel-labs/agent-desktop/compare/v0.1.4...v0.1.5) (2026-02-23)


### Features

* add fallback chains for set-value, clear, focus, scroll-to, type and post-action state hints ([11f8da0](https://github.com/vercel-labs/agent-desktop/commit/11f8da06e84ed67b0e26dbc1946f7a7542e89dcd))
* add structured verbose logging across all layers ([c7316e8](https://github.com/vercel-labs/agent-desktop/commit/c7316e8b5160ab0e6ba554bcd502d4c47adf8b1a))


### Bug Fixes

* add dwell time before drag release for drop target recognition ([2a52d62](https://github.com/vercel-labs/agent-desktop/commit/2a52d62106699b36f62f9af83895f2264b80efb1))

## [0.1.4](https://github.com/vercel-labs/agent-desktop/compare/v0.1.3...v0.1.4) (2026-02-23)


### Features

* add agent-desktop skill for universal AI agent support ([ef45135](https://github.com/vercel-labs/agent-desktop/commit/ef45135087d09a7e065f65d9a0558d1e710cb8bf))
* add Claude Code skills for agent-desktop automation ([ad91cd3](https://github.com/vercel-labs/agent-desktop/commit/ad91cd32cf2de1c1c8dcda4c0dcae37f0022b4c6))

## [0.1.3](https://github.com/vercel-labs/agent-desktop/compare/v0.1.2...v0.1.3) (2026-02-23)


### Bug Fixes

* correct GitHub Release download URL and simplify tag format ([8f66a93](https://github.com/vercel-labs/agent-desktop/commit/8f66a9346e02a751a83ac02313dcec2d9c81bde8))
* include README and CHANGELOG in npm package ([084fc8c](https://github.com/vercel-labs/agent-desktop/commit/084fc8c960527c0d0654028794ec3c4fd2d970c4))


### Performance

* use curl for binary download in postinstall ([ebafb71](https://github.com/vercel-labs/agent-desktop/commit/ebafb71603f5f2b32af8ac5bf6c88df3d6012f70))

## [0.1.2](https://github.com/vercel-labs/agent-desktop/compare/agent-desktop-v0.1.1...agent-desktop-v0.1.2) (2026-02-23)


### Bug Fixes

* use macos-latest for both build targets ([91c7677](https://github.com/vercel-labs/agent-desktop/commit/91c76777cb7ee864b45e14d123c79c08f0c2d5b9))

## [0.1.1](https://github.com/vercel-labs/agent-desktop/compare/agent-desktop-v0.1.0...agent-desktop-v0.1.1) (2026-02-23)


### Features

* 10-step scroll chain, focus guards, enhanced click chain, bounds fix ([595ccb6](https://github.com/vercel-labs/agent-desktop/commit/595ccb6cc45554351ea3e30b95e4ca47bdf4e16b))
* add 19 new commands, AX-first rewrites, LOC compliance ([d3f7e03](https://github.com/vercel-labs/agent-desktop/commit/d3f7e03c67832c652a6125f61fbb7ab2f0801939))
* add 19 new commands, AX-first rewrites, LOC compliance ([eca04e8](https://github.com/vercel-labs/agent-desktop/commit/eca04e839288b121f6f41c6de525a8396d10654c))
* add release automation with GitHub Releases and npm distribution ([18fc50c](https://github.com/vercel-labs/agent-desktop/commit/18fc50cca51f2ed10b6dfb5576602b6ce344bc95))
* add structural hints to splitter columns in snapshots ([48f8470](https://github.com/vercel-labs/agent-desktop/commit/48f8470948b4f636dfa6f4489e4cb6d9f520722c))
* AX-first right-click chain with inline context menu capture ([cddc5d3](https://github.com/vercel-labs/agent-desktop/commit/cddc5d3547f058a78f8b398fa982e39a1fcbf6b1))
* Phase 1 foundation — workspace scaffold, core engine, macOS adapter, 31 commands ([a346f24](https://github.com/vercel-labs/agent-desktop/commit/a346f242c25dfad1c849e6d50f9ab25a42b462d9))
* smart AX-first click chain + macOS crate restructure ([4616c8f](https://github.com/vercel-labs/agent-desktop/commit/4616c8f65f974505b0eedb5485c865d3b905342b))
* surface-targeted snapshot, menu wait, list-surfaces command ([39178b2](https://github.com/vercel-labs/agent-desktop/commit/39178b291602d192de97aa0150c261db1dcc7ca6))


### Bug Fixes

* add menubar surface, fix press --app crash and modifier mapping ([a231962](https://github.com/vercel-labs/agent-desktop/commit/a2319623b4d1d2b6b2f6e1a4ab9a8b8cbbfd02eb))
* address code review findings (double-free, CF leaks, injection) ([2f495ff](https://github.com/vercel-labs/agent-desktop/commit/2f495ffb69be67f3136b076534e078cc31b005c2))
* align error codes with spec (APP_NOT_FOUND, PERM_DENIED) and add -i shorthand ([6dc567a](https://github.com/vercel-labs/agent-desktop/commit/6dc567a4aedff15cf82a82601089cb0b87da4e26))
* ancestor-path cycle detection + CGEvent click fallback ([198d7d7](https://github.com/vercel-labs/agent-desktop/commit/198d7d7d27167044a448b6616fa5c9c0554321bf))
* detect open menus via AXMenuBarItem.AXSelected, not AXMenus attribute ([7f0d610](https://github.com/vercel-labs/agent-desktop/commit/7f0d6103d16969a0abfa84a62b6819dbd0d1cc8e))
* make all 30 commands work end-to-end on macOS ([1d98ab8](https://github.com/vercel-labs/agent-desktop/commit/1d98ab828ce5bcb39e212548ae2f2a052e67aac9))
* remove AXShowDefaultUI from activation chain, fix child walk ([74242f5](https://github.com/vercel-labs/agent-desktop/commit/74242f5040af9c46c98a3f5232dc7567538c28e1))
* resolve all 47 code review findings from Phase 1 audit ([218503a](https://github.com/vercel-labs/agent-desktop/commit/218503a7ebacacd4fbc6b388a6cf5e3bb86af039))
* right-click uses AXShowMenu; context menus detected via focused element ([2c9aee3](https://github.com/vercel-labs/agent-desktop/commit/2c9aee397912d6a903d9ef1e26c786697383ae95))
* suppress dead_code lint on BatchCommand deserializer struct ([608d4aa](https://github.com/vercel-labs/agent-desktop/commit/608d4aaaa195b95626f17aa4bbca2d69609f14cc))
* use simple release strategy for workspace version bumps ([0ab78dd](https://github.com/vercel-labs/agent-desktop/commit/0ab78dde0e1ff702db6c8b667784fa456245b26b))
