# macOS CLI latency (1.4)

Goal: agent-desktop's own time near zero; every command under 100 ms on a warm app, or a measured reason why not.
Mac M2, 8 GB, macOS 26.6. Release build of origin/main 3b49edcf2. Warm = app already running; cold = right after `launch`.
Each cell: one warm-up discarded, then 7 runs (cold: 7 launch cycles). Cells read **min / median / max** in ms, wall time of the CLI process.
Toolkits: AppKit (Calculator, Script Editor), SwiftUI (AgentDeskFixture, System Settings), SwiftUI/Catalyst-style (Weather).
Electron and Chromium were skipped: every installed Electron app holds personal content when opened, and Chrome was not used.

## Phase 1 table

| command | Calculator | Fixture (SwiftUI) | Script Editor | System Settings | Weather |
|---|---|---|---|---|---|
| list-windows | 108 / **113** / 116 | 121 / **129** / 145 | 99 / **118** / 124 | 104 / **104** / 107 | 107 / **111** / 117 |
| snapshot | 172 / **177** / 186 | 256 / **266** / 294 | 159 / **193** / 443 | 419 / **428** / 456 | 341 / **343** / 366 |
| snapshot -i | 171 / **183** / 193 | 262 / **268** / 278 | 179 / **181** / 188 | 420 / **424** / 425 | 317 / **324** / 341 |
| snapshot -i --compact --lines | 174 / **179** / 183 | 251 / **259** / 277 | 173 / **180** / 184 | 416 / **422** / 427 | 321 / **333** / 355 |
| find role+name | 170 / **172** / 179 | 225 / **246** / 275 | 170 / **177** / 190 | 332 / **348** / 351 | 241 / **265** / 281 |
| find text | 190 / **192** / 196 | 803 / **807** / 840 (0/7 ok, TIMEOUT) | 171 / **176** / 184 (0/7 ok, TIMEOUT) | 1279 / **1291** / 1364 | 846 / **862** / 932 |
| click | 73 / **75** / 76 | 132 / **134** / 137 | 189 / **236** / 249 | 174 / **178** / 192 | 459 / **473** / 534 |
| set-value | n/a | 74 / **81** / 93 | 67 / **69** / 83 (0/7 ok, APP_UNRESPONSIVE) | 67 / **68** / 72 | 144 / **153** / 184 |
| press | 221 / **224** / 230 | 248 / **278** / 330 | 226 / **252** / 261 | 226 / **247** / 252 | 226 / **240** / 268 |
| get | 66 / **69** / 71 | 91 / **98** / 99 | 68 / **70** / 74 | 69 / **69** / 73 | 60 / **61** / 68 |
| wait text (true) | 176 / **182** / 186 | 819 / **835** / 946 | 173 / **184** / 231 | 351 / **355** / 360 | 5043 / **5050** / 5059 (0/7 ok, TIMEOUT) |
| cold launch | 591 / **620** / 705 | 491 / **518** / 550 | 549 / **575** / 590 | 785 / **853** / 906 | 1185 / **1272** / 1376 |
| cold snapshot -i | 354 / **403** / 523 | 504 / **513** / 530 | 258 / **273** / 378 | 1018 / **1264** / 1314 | 908 / **947** / 1012 |
| cold find role+name | 296 / **321** / 450 | 423 / **450** / 455 | 228 / **249** / 275 | 1090 / **1172** / 1316 | 737 / **766** / 869 |
| cold close-app | 158 / **163** / 179 | 150 / **153** / 187 | 151 / **158** / 187 | 153 / **168** / 226 | 150 / **159** / 240 |

Calculator has no settable field, so it has no set-value cell. Calculator warm rows come from a rerun, because a parallel cargo build tainted the first pass.

## Where the time goes (10 slowest cells, medians)

start = exec and dyld before our first instruction. tcc = AXIsProcessTrusted. ax = all AX IPC (call count, longest single call). cg = CGWindowListCopyWindowInfo (longest call). sleep = nanosleep (count). fsync = F_FULLFSYNC plus rename (count). ours = the rest.

| cell | median | start | tcc | ax (n, longest) | cg (longest) | sleep (n) | fsync (n) | ours | owner |
|---|---|---|---|---|---|---|---|---|---|
| Weather wait text | 5050 | 14 | 16 | 1856 (5664, 28) | 174 (33) | 2554 (48) | 0 | 485 | ours: 48 × 100 ms polls; the text never matched in this run (TIMEOUT), though it was ok in the diag batch |
| Settings find text | 1291 | 14 | 15 | 1150 (9928, 23) | 38 | 6 | 14 (4) | 51 | app/OS per call, ours in count: about 10k IPCs, a full walk with no early exit |
| Weather cold launch | 1272 | 13 | 13 | 553 (6, 395) | 98 | 395 (13) | 0 | 170 | app start (395 ms single AX call) plus our launch polls |
| Settings cold snapshot -i | 1264 | 218 | 110 | 584 (3623, 39) | 52 | 34 (2) | 12 (4) | 126 | app/OS: the first AX connection to a just-launched app plus the OS loading dyld caches |
| Settings cold find | 1172 | 146 | 53 | 530 (2284, 26) | 52 | 35 (2) | 14 (4) | 271 | app/OS mostly |
| Weather cold snapshot -i | 947 | 17 | 14 | 774 (1250, 92) | 40 | 36 (2) | 14 (4) | 50 | app (AX server is slow while it starts up) |
| Weather find text | 862 | 10 | 13 | 741 (2999, 19) | 34 | 6 | 14 (4) | 42 | as Settings find text |
| Settings cold launch | 853 | 14 | 13 | 367 (11, 236) | 90 | 258 (10) | 0 | 140 | app start plus our launch polls |
| Fixture wait text | 835 | 25 | 17 | 682 (1263, 45) | 36 | 6 | 15 (4) | 54 | locator walk; see find text |
| Fixture find text | 807 | 11 | 13 | 689 (8079, 21) | 39 | 6 | 0 | 49 | app per call, ours in count; ends in TIMEOUT `locator_incomplete` |

The fixed per-process floor, paid by every command and measured with an interposer and ADPERF marks:
- **TCC:** the preflight is about 23 ms (AX trust about 12 ms plus screen-recording about 11 ms), serial. **OS, ours to overlap.**
- **Connections:** the first window-server call is about 30 ms; the first AX message to an app is 23–40 ms. Both are serial on one thread. **OS, ours to overlap.**
- **Stabilizer:** a 5 ms sleep in the stabilizer at `cg_window.rs:116` and `app_inventory.rs:119`; each costs 6.3 ms and press pays 2. **Ours.**
- **Legacy host scan:** `legacy_host.rs` reads KERN_PROCARGS2 of 342 processes on every call, about 2 ms. **Ours.**
- **Snapshot writes:** refmap plus latest-id writes cost 11–26 ms per snapshot. That is 4 × F_FULLFSYNC at about 3 ms each; an atomic file plus directory flush is 6.0 ms median on this disk, while plain fsync is 0.07 ms. **Ours (core, laptop-owned).**
- **Settled-surface walk:** after a click, the context-menu BFS in `app_menu.rs` costs Calculator 17–19 ms, Fixture 52–60, Script Editor 28–37, Settings 108–120 and Weather 308–322. **Ours.**
- **Action dispatch:** the AXPress round trip is Calculator 3–4 ms, Fixture 14–16, Script Editor 103–107, Settings 1 and Weather 9–12. **App.**
- **Hit test:** AX returns unknown in 0.55–1.34 ms (control's numbers). Laptop-owned, not edited.

## Fixes (ranked by ms saved)

Measured by interleaved A/B against origin/main on 5 apps; medians.

- [x] 1. Overlap OS connections and TCC checks (#95, merged 16a252506). Saves 25-38 ms on `list-windows`, 26-40 on `snapshot -i`, 28-42 on `find`, 19-37 on `press`, 9-13 on `get`/`set-value`/`click`.
- [x] 3. Skip the second child count on leaves (#97, merged 272c08378). Saves 10-17 ms on the fixture and Weather, 2-4 elsewhere; trees identical.
- [x] 5. Stabilizer recapture without the 5 ms pause (#98). Saves 3-7 ms on `list-windows`, 3-16 on `press`. Merged e49b2295f.
- [x] 4. Legacy host scan reads identity first (#96). Saves 1.5 ms on every command (`--version` 7.72 to 6.19 ms).
- [x] FRONT-READ-COLD-1 (#99). The first system-wide focus read failed -25204 in every fresh process because there was no window-server connection yet. Classification is now correct, at +6 ms (front) and +35 ms (behind) per headed pointer command on the probe branch.
- [x] LAUNCH-WIN-1 (#100, merged 3f593469f). `launch` now reports the window once it is placed: real bounds and visible in 16/16, at +56-160 ms per launch.
- [x] 6. Launch poll granularity: poll cap 250 to 50 ms, folded into #100. Saves 4/16/18/182 ms on the four launch apps.
- [x] 2. Context-menu walk after click-like actions (#101). `AXRole`+`AXChildren` in one batched read per element; `AXVisible` only for menus (batching it made Weather 8-47 ms slower); any unclear slot falls back to the old reads. `click` medians: System Settings -40/-51 ms, Weather -56/-89, fixture -8, Calculator -1, Script Editor 0/+3. Also fixes a `read_owned` leak of a value that arrives after the deadline.

## Found (for core / laptop)

- `find --text` fails TIMEOUT `locator_incomplete` on Fixture (8 matches seen, 669 ms) and Script Editor (57 ms) while `find` by role and name works.
- `wait --text` on an already-true condition costs what `find --text` costs (Fixture 835 vs 807 ms): the text locator walks the whole tree. On Weather, `wait --text` timed out 7 of 7 at 5 s in phase 1 while `find --text` matched; it passed in the diag batch, so this is not confirmed.
- Script Editor `set-value` fails APP_UNRESPONSIVE, "Accessibility surface read failed for AXRole" (-25202), 7 of 7.
- With a full-screen app in front, `launch` took 2.25 s and the next `snapshot` failed WINDOW_NOT_FOUND in 0.18 s; AX sees only the current Space.
- There are 4 F_FULLFSYNC per snapshot (`private_file.rs`, laptop-owned); see the snapshot-writes item above.
- The FRONT-READ root cause also explains why later reads in the same process succeed: something had listed windows by then. `AXUIElementCopyElementAtPosition` works cold and opens the connection itself, so `hit_test.rs` is not affected.
- Windows ARM64 CI flake, unrelated to macOS: `context::scope_tests::a_command_does_not_trace_into_a_session_reaped_while_it_waited_for_its_lease` failed with `DirectoryNotEmpty` (os error 145) on #95's first run.
- `press` after #95/#98/#100 is 132-145 ms process time (167-191 ms wall) on Calculator and Weather. Breakdown on Calculator: AX trust 14 ms; app inventory about 37 ms (4 run-loop spins about 9 ms apart); first AX read to the app 30-46 ms (connection setup, already overlapped by #95); about 33 ms of one-time OS setup on the main thread inside the first `CGEventCreateKeyboardEvent` (SkyLight event init about 10 ms, TSM/LaunchServices check-in about 18 ms); the first post's TCC check 10.6 ms (a once-per-process call inside `postEvents`); `KEY_SETTLE` 5 ms. Probe (`kbd.c`, no events posted): a cold key-event build costs 39 ms on the main thread; if a background thread builds one about 25 ms earlier, the main thread pays 13-17 ms, a saving of about 24 ms. Off-main creation did not crash in 6/6 fresh processes. **Core item:** the only keyboard-specific adapter call that comes before the AX reads is `is_blocked_combo`. It is a predicate and is skipped under `--force`, so starting a warm-up there would be a hack. A `PlatformAdapter` hint such as `prepare_keyboard_input()`, with a no-op default and called first by `press`, `type` and the key commands, would let macOS start a `connection_warmup` thread that builds one unposted key event. Expected about 24 ms per keyboard command. The 10 ms post TCC check cannot be warmed without posting.
