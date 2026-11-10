# macOS CLI latency, round 3 (1.4)

Bar: every command under 100 ms on a warm app, or a measured reason. Mac M2 8 GB, macOS 26.6.
Method as round 2 (t3/macos-cli-latency-round2:TASKS.md): release builds, interleaved A/B, one warm-up, 7 pairs, min/median/max.
New: `axattr.dylib` interposer counts every AX call per (function, attribute) and distinct elements; `AXATTR_SEQ` logs each call in order.

## Items

- [x] A. AX call count per node. Child reads counted, read, counted again. #110 merged e9a42b7f0: over-ask by one proves the count; container roles read values first.
  Calls/node (snapshot): Calculator 6.0→5.5, fixture 5.6→5.3, Script Editor 6.5→6.0, System Settings 10.6→9.1, Weather 6.1→5.5.
  Medians: Settings −40 to −48 ms, Weather −24 to −27, fixture −6 to −7, Calculator/Script Editor flat.
  Rejected: values-first for every role (+3 to +7 ms on the fixture: leaves pay a refused page plus a count).
- [x] B. find/wait --text: extra AX calls already gone on main (find --text = snapshot calls since #105).
  Weather `wait --text Humidity` 0/7 was not a bug: after a set-value on Weather's Search field the window has 31 nodes and no
  "Humidity" (independent AX walk 0 hits; 1 hit in 91 nodes before). Round 2 counted find's `ok` with `total_matches: 0` as a match.
- [ ] C. #112 open, green, waits for laptop #86 (touches app_menu.rs): the window pass skips the focused subtree the focused pass finished.
  Weather click 286→203 AX calls, −21 ms median, max 455→202. Script Editor −15.
- [x] D. #113 merged b94ad8976: ref commands start the app's AX warm-up from the refmap pid before the TCC check; press --ref also warms the window server in the background.
  Medians: get −12 to −15 ms (5 apps), click −9 to −19 (Weather −3, settle-bound), press --ref −30 to −35 headless and headed (Calculator 159→126, Script Editor 161→131).
  Rejected: early keyboard-layout load before the preflight (stalls main 33 ms: window-server connection not yet open).
- [ ] E. #114 open: #108 follow-ups (live app-list tests retry until the list is stable; JSON-driven every_app closure test; workspace_apps.rs / appkit_bridge.rs untouched, so no split needed).
  #115 open (waits for #86: menu_search_node.rs): #101 follow-ups (batch asked once before the attribute fallback; read() error paths tested through an injected copy; child-limit test on real AXUIElements). Open: the `None => inspect_by_attribute` arm test needs app_menu.rs, which #112 changes: after #112 merges.
- [x] F. Persistent-process sizing (measure only, main e9a42b7f0): one `batch` process runs 1 vs 11 entries; per request = (t11 - t1)/10. Process floor (`version`) 7.6 ms.
  | App | get CLI / in-process | click CLI / in-process | list-windows CLI / in-process | snapshot -i CLI / in-process |
  |---|---|---|---|---|
  | Calculator | 40.3 / 1.7 | 56.5 / 7.1 | 68.9 / 23.2 | 95.9 / 38.1 |
  | AgentDeskFixture | 45.1 / 2.3 | 86.5 / 35.1 | 71.5 / 23.1 | 159.7 / 110.5 |
  | Script Editor | 41.1 / 0.9 | 243.7 / 138.7 | 69.9 / 23.1 | 95.7 / 39.6 |
  | System Settings | 49.7 / 1.9 | 128.1 / 92.7 | 85.8 / 30.0 | 281.5 / 207.6 |
  | Weather | 45.7 / 3.2 | (not idempotent) | 73.0 / 25.0 | 212.1 / 160.1 |
  A persistent process saves 35-75 ms per command (process start, TCC, the app's first AX reply, window-server connection). Medians of 5 reps.
  Found: in-process `list-windows` still costs 23-30 ms per call: inventory work, not process setup. Worth a profile next round.
- [ ] G1. close-app exit poll: #116 open. kqueue NOTE_EXIT ends the 25 ms pause; exit-to-return lag max 18.7 ms (main) -> 2.4 ms (branch) on 4 apps; wall within noise (app quit dominates).
- [ ] G2. launch settle polls: #117 open. Poll 10 ms rising to a 25 ms cap (was 25 -> 50). Final pause before the returning read 53-55 -> 29-30 ms median on cold Calculator/fixture/Weather; wall medians -7 to -49 (noisy).
  Cold launch is dominated by the app: its first AX reply blocks 250-560 ms while its main thread starts up.
- [x] G3. Full-screen Space launch: measured reason, no change. With a full-screen app in front, `launch Calculator` returns ok without a window after 1.8-2.4 s (20-25 sleeps, 41-51 window reads).
  The window exists only as an off-screen CG record (layer 0, 230x408, kCGWindowIsOnscreen absent); AXWindows lists none (AX lists current-Space windows only).
  `narrow_to_real_windows` cannot tell it from an ordered-out helper window without private Space APIs, so the wait ends at the 1.5 s startup grace by design. `--activate` is the documented way to get the window.

- [ ] H. (found via F) scoped window list read every app's AppKit record twice: #119 open. Named owner read; list-windows --app -6 to -11 ms, snapshot/find/focus-window -4 to -11 on 5 apps; parity 25/25 plus alias/not-found.
  Left: unscoped list-windows and other full owner reads (global inventory, actions/physical_target (waits for #86), notifications).

## Found

- close-app on System Settings returns 10-12 ms before the kernel reports the pid's exit (both binaries): the identity check stops matching while the process is still exiting, so `Exited` is claimed slightly early.
- CI flake: Windows ARM64 `opening_the_start_menu_round_trips_into_the_snapshot` failed once with UIA E_UNEXPECTED on #113 (no Windows code touched). Local flake: `a_marked_copy_running_version_exits_successfully` failed once, passed alone and on rerun.

- press --ref on targets that take no keyboard focus fails on main and branch alike: fixture button headless ACTION_FAILED "lost focus" / headed TIMEOUT ~600 ms; System Settings cell AXFocused kAXErrorFailure; Weather button TIMEOUT ~600 ms (focus waits of 6 ms x ~79).

- System Settings `click` runs the surface inventory twice: the menu chain's open-menu check after the press, then the settled-surface report. Menu-chain files belong to open PR #86. Owned there.
- The SwiftUI fixture's `right-click` on a text field takes ~830 ms and reports no surface (both main and branch).
- Local: `committed_header_compiles_as_cpp17_with_portable_abi_types` fails on this Mac (`'cstdint' file not found`): command-line tools lack C++ headers. CI covers it.
- The first AX call of every process to Weather costs ~72 ms (app side, per process): one more argument for item F.
