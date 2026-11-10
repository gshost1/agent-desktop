# macOS CLI latency, round 2 (1.4)

Bar: every command under 100 ms on a warm app, or a measured reason why not.
Mac M2, 8 GB, macOS 26.6. Release build of origin/main **7cea97c18** (contains #95 #96 #97 #98 #100 #101; #99 still open).
Warm cells: one warm-up discarded, 7 runs, min / **median** / max wall ms of the CLI process. Cold rows (launch, first snapshot/find,
close-app) are from 3f593469f, whose launch and walk code matches 7cea97c18.

## Floor table (warm)

Budget columns come from the fastest of two profiled runs (interposer on every AX, CG window-list, sleep, fsync, run-loop and
event-post call; main thread only, warm-up threads excluded). **start** = exec + dyld + exit of this binary (`--version`, 5.7 ms;
OS, 1.9 ms of it is bare process spawn). **tcc** = AXIsProcessTrusted on the main thread (OS). **cg** = window-server list calls,
first one opens the connection (30-40 ms, OS). **ax** = AX IPC on the main thread, call count in brackets (app). **ev+rl** =
CGEventPost plus NSWorkspace run-loop spins (OS). **fsync** = F_FULLFSYNC + rename for refmap/latest-id writes (ours, core
`storage/private_file.rs`). **sleeps** = fixed waits (ours). **ours** = the remainder: our CPU and untracked syscalls.
**floor** = start + ax + ev+rl + max(tcc, cg) — what is left if all of ours went to zero and TCC overlapped the window-server
connection. Floors carry about ±5 ms of interposer inflation.

| cell | median (min/max) | ok | start | tcc | cg | ax (n) | ev+rl | fsync (n) | sleeps | ours | floor 
|---|---|---|---|---|---|---|---|---|---|---|---
| AgentDeskFixture click | **89** (88/93) | 7/7 | 5.7 | 12 | 0 | 66 (294) | 0 | 0 (0) | 0 (0) | 0 | **84** |
| AgentDeskFixture find role+name | **169** (166/177) | 7/7 | 5.7 | 11 | 36 | 73 (665) | 3 | 12 (4) | 0 (0) | 18 | **118** |
| AgentDeskFixture find text | **627** (624/636) | 0/7 | 5.7 | 12 | 36 | 531 (6732) | 3 | 0 (0) | 0 (0) | 29 | **575** |
| AgentDeskFixture list-windows | **74** (71/130) | 7/7 | 5.7 | 0 | 40 | 2 (6) | 3 | 0 (0) | 0 (0) | 14 | **50** |
| AgentDeskFixture press | **166** (155/186) | 7/7 | 5.7 | 12 | 30 | 31 (22) | 15 | 0 (0) | 5 (1x4ms) | 40 | **82** |
| AgentDeskFixture set-value | **52** (50/54) | 7/7 | 5.7 | 12 | 0 | 28 (74) | 0 | 0 (0) | 0 (0) | 0 | **45** |
| AgentDeskFixture snapshot -i | **195** (194/197) | 7/7 | 5.7 | 11 | 34 | 101 (1099) | 3 | 13 (4) | 0 (0) | 20 | **143** |
| AgentDeskFixture snapshot | **195** (192/198) | 7/7 | 5.7 | 12 | 35 | 100 (1099) | 3 | 12 (4) | 0 (0) | 20 | **144** |
| AgentDeskFixture wait text (true) | **195** (189/204) | 7/7 | 5.7 | 12 | 36 | 100 (1099) | 3 | 14 (4) | 0 (0) | 17 | **144** |
| Calculator click | **60** (60/62) | 7/7 | 5.7 | 12 | 0 | 50 (140) | 0 | 0 (0) | 0 (0) | 0 | **68** |
| Calculator find role+name | **126** (125/128) | 7/7 | 5.7 | 11 | 35 | 34 (127) | 3 | 10 (4) | 0 (0) | 16 | **77** |
| Calculator find text | **149** (140/151) | 7/7 | 5.7 | 11 | 36 | 55 (683) | 2 | 12 (4) | 0 (0) | 16 | **99** |
| Calculator list-windows | **72** (67/74) | 7/7 | 5.7 | 0 | 39 | 1 (6) | 3 | 0 (0) | 0 (0) | 15 | **49** |
| Calculator press | **175** (162/180) | 7/7 | 5.7 | 13 | 30 | 38 (21) | 15 | 0 (0) | 4 (1x4ms) | 38 | **89** |
| Calculator snapshot -i | **129** (128/134) | 7/7 | 5.7 | 11 | 35 | 35 (210) | 3 | 13 (4) | 0 (0) | 15 | **78** |
| Calculator snapshot | **128** (126/134) | 7/7 | 5.7 | 11 | 36 | 36 (210) | 3 | 11 (4) | 0 (0) | 15 | **81** |
| Calculator wait text (true) | **130** (127/138) | 7/7 | 5.7 | 11 | 36 | 36 (210) | 3 | 12 (4) | 0 (0) | 15 | **80** |
| Script Editor click | **226** (191/246) | 7/7 | 5.7 | 16 | 0 | 157 (133) | 0 | 0 (0) | 0 (0) | 0 | **179** |
| Script Editor find role+name | **147** (124/149) | 7/7 | 5.7 | 11 | 35 | 33 (133) | 3 | 10 (4) | 0 (0) | 15 | **77** |
| Script Editor find text | **136** (133/138) | 0/7 | 5.7 | 11 | 35 | 54 (918) | 3 | 0 (0) | 0 (0) | 18 | **98** |
| Script Editor list-windows | **73** (67/77) | 7/7 | 5.7 | 0 | 35 | 1 (6) | 3 | 0 (0) | 0 (0) | 16 | **45** |
| Script Editor press | **177** (176/184) | 7/7 | 5.7 | 12 | 29 | 52 (25) | 15 | 0 (0) | 5 (1x4ms) | 37 | **102** |
| Script Editor set-value | **53** (48/58) | 0/7 | 5.7 | 11 | 0 | 29 (1) | 0 | 0 (0) | 0 (0) | 0 | **46** |
| Script Editor snapshot -i | **134** (126/138) | 7/7 | 5.7 | 11 | 35 | 33 (235) | 3 | 14 (4) | 0 (0) | 16 | **76** |
| Script Editor snapshot | **128** (122/223) | 7/7 | 5.7 | 11 | 35 | 33 (235) | 3 | 12 (4) | 0 (0) | 18 | **76** |
| Script Editor wait text (true) | **128** (121/133) | 7/7 | 5.7 | 11 | 35 | 38 (204) | 3 | 16 (4) | 0 (0) | 18 | **82** |
| System Settings click | **99** (98/100) | 7/7 | 5.7 | 12 | 0 | 72 (382) | 0 | 0 (0) | 0 (0) | 0 | **90** |
| System Settings find role+name | **249** (247/260) | 7/7 | 5.7 | 14 | 38 | 157 (1075) | 3 | 12 (4) | 0 (0) | 15 | **203** |
| System Settings find text | **994** (985/1003) | 7/7 | 5.7 | 16 | 39 | 881 (7759) | 3 | 13 (4) | 0 (0) | 24 | **928** |
| System Settings list-windows | **97** (78/215) | 7/7 | 5.7 | 0 | 40 | 2 (6) | 3 | 0 (0) | 0 (0) | 18 | **49** |
| System Settings press | **168** (148/172) | 7/7 | 5.7 | 12 | 30 | 21 (22) | 15 | 0 (0) | 5 (1x4ms) | 36 | **72** |
| System Settings set-value | **52** (51/52) | 7/7 | 5.7 | 12 | 0 | 30 (115) | 0 | 0 (0) | 0 (0) | 0 | **47** |
| System Settings snapshot -i | **295** (290/307) | 7/7 | 5.7 | 13 | 38 | 200 (1907) | 3 | 10 (4) | 0 (0) | 17 | **246** |
| System Settings snapshot | **305** (296/320) | 7/7 | 5.7 | 13 | 35 | 200 (1907) | 3 | 13 (4) | 0 (0) | 20 | **243** |
| System Settings wait text (true) | **293** (288/301) | 7/7 | 5.7 | 13 | 35 | 198 (1907) | 3 | 12 (4) | 0 (0) | 19 | **241** |
| Weather click | **222** (175/410) | 7/7 | 5.7 | 14 | 0 | 194 (290) | 0 | 0 (0) | 0 (0) | 0 | **214** |
| Weather find role+name | **190** (188/199) | 7/7 | 5.7 | 13 | 37 | 96 (288) | 3 | 14 (4) | 0 (0) | 15 | **141** |
| Weather find text | **699** (690/734) | 7/7 | 5.7 | 19 | 41 | 571 (2481) | 3 | 8 (4) | 0 (0) | 14 | **620** |
| Weather list-windows | **76** (69/81) | 7/7 | 5.7 | 0 | 37 | 1 (6) | 3 | 0 (0) | 0 (0) | 13 | **47** |
| Weather press | **149** (142/158) | 7/7 | 5.7 | 19 | 39 | 27 (21) | 15 | 0 (0) | 5 (1x4ms) | 26 | **86** |
| Weather set-value | **126** (123/131) | 7/7 | 5.7 | 14 | 0 | 95 (95) | 0 | 0 (0) | 0 (0) | 0 | **115** |
| Weather snapshot -i | **267** (263/274) | 7/7 | 5.7 | 14 | 38 | 162 (565) | 3 | 15 (4) | 0 (0) | 15 | **208** |
| Weather snapshot | **266** (263/280) | 7/7 | 5.7 | 12 | 39 | 168 (565) | 3 | 15 (4) | 0 (0) | 14 | **216** |
| Weather wait text (true) | **5056** (5040/5062) | 0/7 | 5.7 | 14 | 225 | 1812 (4705) | 57 | 0 (0) | 2166 (1x100ms+20x99ms) | 740 | **2100** |

Reading the floor: every cell's remaining time is the app's AX server, the OS connection setup, or the OS permission check.
Cells whose floor is over 100 ms are over because of AX time inside the app (Settings 1907 calls / 200 ms for one walk, Weather
single calls of 40-80 ms while it updates) — except where the AX **count** is ours:

- **find text / wait text**: 2481-7759 AX calls against 288-1075 for role+name on the same app. The text locator reads every
  node's text attributes over the whole tree (core locator + macOS text reads), and fails `TIMEOUT locator_incomplete` on the
  fixture and Script Editor. Weather `wait --text Humidity` never matches (0/7, 5 s TIMEOUT, 20 × 100 ms polls) while
  `find --text Humidity` does (7/7).
- **click / press settled surface**: the raised-surface walk after the action (core `ref_action/settled_surface.rs`): Calculator
  21 ms, fixture 31, Script Editor 25, Settings 50, Weather 51-229 of the click.
- **press**: 37-47 ms ours: four app-inventory reads (~9 ms each, NSWorkspace run-loop spins) and the first key-event build
  (~9 ms TSM/LaunchServices check-in); plus a 4 ms `KEY_SETTLE` sleep and the 30 ms window-server connection on the main thread.

## Cold rows (3f593469f, 7 launch cycles)

| app | launch | first snapshot -i | first find | close-app | launch sleeps (polls) | launch longest single AX call |
|---|---|---|---|---|---|---|
| Calculator | 612 / **636** / 646 | 256 / **293** / 336 | 243 / **254** / 262 | 124 / **127** / 148 | 173 ms | 214 ms |
| Fixture | 512 / **536** / 576 | 416 / **435** / 445 | 344 / **359** / 371 | 120 / **125** / 131 | 125 ms | 188 ms |
| Script Editor | 597 / **610** / 712 | 213 / **234** / 244 | 199 / **205** / 208 | 127 / **131** / 140 | 177 ms | 185 ms |
| System Settings | 801 / **844** / 987 | 1054 / **1349** / 1483 | 981 / **1146** / 1850 | 131 / **150** / 163 | 180 ms | 257 ms |
| Weather | 1268 / **1334** / 1423 | 873 / **887** / 941 | 717 / **734** / 770 | 129 / **141** / 201 | 440 ms | 409 ms |

Cold first snapshot/find and close-app each pay one 25 ms fixed sleep. Launch pays 25+37.5+50×n ms launch-settle polls
(`system/launch_settle.rs:10,15`) while the app's AX server answers its first calls in 165-409 ms (app start).

## Found

- **The web wrapper drops the preflight report.** `WebChannelAdapter` (`crates/web/src/channel/system_ops.rs`) forwards
  `permission_report` but not `preflight_permission_report`, so every command runs the full report (Automation probe included)
  and the macOS window-server warm-up from #95 never starts: the first window-list call runs on the main thread (30-40 ms) after
  the 11-13 ms TCC check instead of beside it.
- Script Editor `set-value` fails 7/7 `APP_UNRESPONSIVE`, `ax_error` -25202 on `AXRole`, `resolution_phase: remote_token`
  (envelope saved in the scratchpad, `se-setvalue-envelope.json`).
- Hit test before any window listing (#99 review): unknown on attempt 1 was 0/10 on main and 0/10 with the change, fixture and
  Calculator; no window-list call precedes the hit test in either.

## Fixes

- [x] #99 review: hit test through `system_wide::element()`, trusted-or-skip test (pushed 1015100e9).

## Round 2 fixes (2026-11-10)

- [x] #104 window-server connection beside the permission check — merged 8a8dbe449
- [x] #105 find --text publishes from one window read — merged d45c64e31
- [x] #106 element-valued AXValue reads as no value (find --text TIMEOUT on tab/radio groups) — merged f46bd9907
- [ ] #107 fsync instead of F_FULLFSYNC for private state files (-6 to -26 ms snapshot/find/wait) — green before main merge, re-CI
- [ ] #108 scoped app lookup reads full records only for named apps (press -10 to -19, close-app -14 to -23) — re-CI
- [ ] #109 keyboard layout loaded while the app's first AX reply is pending (press -3 to -28) — CI pending
- [ ] press --ref (headed 115 ms): start the AX warm-up from the refmap pid and the window-server hint for headed ref presses; the 9.9 ms layout load and the 28 ms CG connect sit on its main thread
- [ ] close-app 25 ms exit poll (`system/app/app_ops.rs:155-157`)
- [ ] launch settle polls 25/37.5/50 ms (`system/launch_settle.rs`)
- [ ] settled-surface walk after click (21-229 ms)
- [ ] #101 follow-ups (menu_search_node child-limit test, read() fallback tests, batch bound)
- [ ] full-screen Space launch: ok:true without a window after ~1.8 s (22 sleeps, 46 CG calls)
- Found: thread-built key events carry no characters, so keyboard warm-up must run on the main thread.
