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
- [ ] C. settled-surface walk after click. #112 open: the window pass skips the focused subtree the focused pass finished.
  Weather click 286→203 AX calls, −21 ms median, max 455→202. Script Editor −15.
- [ ] D. press --ref warm-up from the refmap pid.
- [ ] E. #101 follow-ups + #108 review follow-ups (live app-list tests tolerant; every_app closure JSON test; split workspace_apps.rs 397 / appkit_bridge.rs 390 first).
- [ ] F. persistent-process sizing probe (measure only).
- [ ] G. close-app exit poll, launch settle polls, full-screen Space launch.

## Found

- System Settings `click` runs the surface inventory twice: the menu chain's open-menu check after the press, then the settled-surface report. Menu-chain files belong to open PR #86. Owned there.
- The SwiftUI fixture's `right-click` on a text field takes ~830 ms and reports no surface (both main and branch).
- Local: `committed_header_compiles_as_cpp17_with_portable_abi_types` fails on this Mac (`'cstdint' file not found`): command-line tools lack C++ headers. CI covers it.
- The first AX call of every process to Weather costs ~72 ms (app side, per process): one more argument for item F.
