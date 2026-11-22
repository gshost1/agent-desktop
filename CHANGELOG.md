# Changelog

## [1.6.0](https://github.com/gshost1/agent-desktop/compare/v1.5.0...v1.6.0) (2026-11-22)

### Features

* A snapshot of 80 or more nodes, including a `--surface` snapshot, starts with `data.contents`, an index of what the tree holds and where: the commonest ref roles by count and, for each list, listbox, table, outline, grid, list-like scroll area, toolbar and tab bar, its line in the tree, its ref, how many items it has loaded and whether platform-reported vertical overflow, mixed showing and not-showing rows, or a snapshot cut means it may hold more. Regions are largest first, and non-partial lists under three items are omitted. A read cut short, or one that stops at a long list, still shows what exists past it. It costs about 300 bytes; `--lines` prints it as lines. ([#219](https://github.com/gshost1/agent-desktop/pull/219))
* **linux:** `mouse-click`, `mouse-move`, `mouse-wheel`, `hover --xy` and `drag --from-xy --to-xy` now work on GNOME Wayland through Mutter RemoteDesktop and ScreenCast, reported `delivered_unverified`, in the desktop space of `list-displays` across monitors, scales and both Mutter layout modes (one click 171.6 ms, min of 7); pointer input aimed at a ref still refuses with `wayland_session`, and a Wayland session without those interfaces refuses with `PLATFORM_NOT_SUPPORTED`, `wayland_interface_missing` ([#220](https://github.com/gshost1/agent-desktop/pull/220))
* On macOS, snapshot contents read a vertical scrollbar's enabled state as evidence that a region scrolls or fits; an absent bar or unreadable state remains unknown.
* macOS: `select @<row> "<label>"` selects a table or outline row and can replace the current selection. If the row is already selected, it makes no write and returns `not_delivered`.

### Bug Fixes

* **linux:** an app started through one of its desktop entry's actions (`gnome-text-editor --new-window`) or by session-bus activation (`gnome-text-editor --gapplication-service`, how GNOME Shell starts a `DBusActivatable` app) now answers to the entry's `Name` and id, so `--app "Text Editor"`, `list-apps --app` and a scoped `wait --event window-opened` find it; before, it answered only to its executable name and a scoped wait timed out with an empty baseline ([#223](https://github.com/gshost1/agent-desktop/pull/223))
* **linux:** `focus-window --window-id` on X11 now refuses a window whose modal dialog is already the active window before any activation request is sent: `ACTION_FAILED`, cause `blocked_by_modal_dialog`, `not_delivered`, `retry: safe`; before, the request was sent first, the parent was raised beneath the dialog, and the refusal was `delivered_unverified` and unsafe to retry ([#224](https://github.com/gshost1/agent-desktop/pull/224))
* On Windows, a `set-value` or `clear` whose classic edit does not answer `EM_REPLACESEL` in time no longer keeps its text buffer for the rest of the process. An edit in another process is still sent inline with `SMTO_ABORTIFHUNG`, because the system marshals the text into that process. An edit on another thread of the calling process (an FFI host's own window) gets the text from a worker thread that owns it until the edit reads it or the edit's thread exits. The worker holds its own module reference so the FFI host can release its library handle while the send is pending. At most 8 such writes wait per process, and a write past that is refused with `not_delivered` before anything is sent. ([#225](https://github.com/gshost1/agent-desktop/pull/225))
* Linux: a snapshot withholds `Select` on an index-only list, row, tab list or tab when the application's toolkit read answers without a supported toolkit, matching the headless action's `selection_index_unproven` refusal. It keeps `Click` on an entry unless GTK 4 is identified, and keeps all tokens when the toolkit read times out or is not attempted, so a fresh live read can judge the route at action time. `docs/json-output.md` now says what `available_actions` promises on each platform ([#227](https://github.com/gshost1/agent-desktop/pull/227))
* On Windows, `dismiss-notification --key` on one of two notifications with the same app, title and body no longer deletes the other one. The dismiss was verified by no identical entry remaining, so the untouched twin made it look ignored and, as the only entry left, triggered the clear-all fallback, which removed the twin and reported success. A dismiss now counts as done when the identical entries drop in number, and the clear-all fallback runs only for a target that had no identical twin; otherwise an ignored dismiss is `ACTION_FAILED` `delivered_unverified` ([#230](https://github.com/gshost1/agent-desktop/pull/230))
* Linux `notification-action` now marks every error after its expand press, including a target that moved when a new notification arrived, with `retry: unsafe` and look-first advice, instead of `not_delivered`; the moved entry is still never pressed. A notification mutation whose message-list open may have pressed the clock menu is unsafe to retry and keeps the shell's `list-surfaces` advice instead of "open the message list by hand and retry", which now appears only on a `not_delivered` open that pressed nothing. ([#231](https://github.com/gshost1/agent-desktop/pull/231))
* `--wait-for` returns a match deeper than the snapshot depth: the wait reads 50 levels down but returned a snapshot cut at 10 and rechecked it, so `click <ref> --wait-for 'button:Done'` with the button 22 levels down timed out on a window that already held it; the returned snapshot now reaches the match and the text that matched it, and a `--skeleton` snapshot becomes a full tree to that depth. ([#234](https://github.com/gshost1/agent-desktop/pull/234))
* A menu bar ref's `--wait-for` in an app with no focused window reads the front document window, the application's main window where the platform names one, instead of a floating panel listed above it, so a selector the menu command shows in the document no longer times out. ([#234](https://github.com/gshost1/agent-desktop/pull/234))
* Suggest exact running app names and PIDs when an app name is nearly right, without acting on a near match.
* `wait --download` no longer reports TIMEOUT for a finished download when a slow host takes over 500 ms to open the download ledger lock; the ledger lock now waits up to 2 s.
* Linux: a sibling destroyed while an unnamed editable field's composite label is read, shown by its own failed role read, counts as absent, so the field stays certainly unlabelled instead of reading as an uncertain name and a native read failure; a live sibling that refuses a state or interface read leaves the label unknown.
* Linux: unnamed editable fields inside web documents no longer take a composite label from their parent's relations or a sibling combo box.
* Recognize GTK 2's GAIL accessibility bridge on X11 so headed ref clicks can use the existing pointer receiver checks.
* Linux snapshots and ref resolution ignore zero-area menubars and their descendants, so an unpresented copy no longer makes visible menu refs ambiguous.
* Linux: `launch` prefers a new descendant with an accessible window over a windowless descendant registered first, such as a wrapper's helper or a restarting process, and reports `AMBIGUOUS_TARGET` when two descendants have windows. When no descendant has a window, a windowed process matching the launched executable outside the spawned process tree can own the launch. Windowless candidates wait until near the launch deadline, with descendants preferred over outside identity matches.
* Linux: `launch` waits the full launch deadline for a process that maps a window but never joins the accessibility bus, instead of returning after about 7 seconds.
* Linux: `launch` returns a verified running windowless owner without a window when its window wait expires after a complete empty read, rather than replacing that observation with a deadline error. Unanswered window reads still fail closed.
* Linux: the off-bus `launch` suggestion names the largest normal window, skipping splash screens and any window that closes mid-read, and suggests `wait --event window-opened` when the title cannot be matched reliably. It no longer creates X11 atoms.
* Linux: `launch` waits until its timeout for an off-bus process even after its native window maps, recognizes new accessible descendants of wrapper processes as launch owners, and suggests a quoted state wait when an off-bus window has a readable title.
* Return `TIMEOUT` for incomplete Linux menu reads, require menu-typed popup evidence fully containing at least one readable item, and tolerate items or X11 windows disappearing during a read.
* Detect open Linux menus when scrolled-out, screen-edge-clipped, or stale item bounds lie outside the popup.
* Preserve completed empty Linux menu reads at deadline expiry, treat vanished owners as absent, and retain original read errors and observed popups after partial inventory reads.
* Retry newly mapped Linux menu windows until a complete accessibility read has run while that window was already mapped, without trusting stale menu-item bounds.
* Keep newly mapped Linux menu windows pending for at least 175 ms and a completed inventory read, restart settling after reappearance, and reconnect once after a failed X11 popup read.
* Detect open Linux menus whose items expose VISIBLE without SHOWING, using valid screen bounds corroborated by an app-owned mapped X11 popup so stale closed-menu bounds do not count as an open menu.
* Retry an initially absent Linux menu surface for up to 500 ms only while an unclassified app-owned mapped X11 menu window provides opening evidence; closed menus and non-X11 sessions add no retry delay.
* Limit structural menu bounds reads to items below a SHOWING nearest menu, and share native popup corroboration across a top-level window's children.
* **linux:** Resolve active dialogs whose accessible and native titles differ, and briefly re-read conflicting window focus evidence for app-scoped commands while preserving refusal for background applications.
* Stop advertising headless Select for Qt dropdowns and their popup choices, whose selection routes require explicit pointer permission; headed selection remains available.
* Open collapsed Qt combos before headed selection, then find the choice again and verify its committed value instead of attempting to click a hidden popup row.
* Refuse before input when a Qt combo does not advertise expansion; otherwise reserve cleanup time before opening and send Escape if expansion cannot be observed, retaining the unsafe retry disposition after input.
* Send cleanup Escape only to an observed visible popup, and retain unknown popup state instead of treating an unread list as proof that it stayed open.
* Recheck popup visibility immediately before cleanup Escape and skip the key if the popup has closed or become unreadable.
* Report a popup close attempt only when cleanup sent Escape, not when its visibility recheck skipped the key.
* Close Qt combo popups when choosing a row fails after opening, preserving opener input accounting and reporting whether cleanup sent Escape and observed closure.
* Give unobserved Qt combo popups the same recovery advice and Activities Overview diagnosis as other pointer-opened dropdowns.
* Keep the popup cleanup reservation while searching for and verifying a choice after opening; use the full remaining deadline for cleanup.
* Name the Activities Overview only when the popup is observed absent, never when it is visible or unreadable.
* Pace X11 modifier chords so Qt accessibility processing does not close menus opened by Alt mnemonics.
* Match Linux X11 dialogs by same-process geometry, including all four window-manager decoration edges, when native and accessibility titles differ, restoring dialog bounds, focus, and button actions while preserving ambiguous matches.
* Briefly recheck X11 activation while no active process can be identified, avoiding transient dialog activation refusals without accessibility registry reads or waits when another process owns the active window.
* Keep native activation and accessibility focus rechecks within one shared 300 ms budget, and preserve geometry ambiguity when reconciling unmatched active dialogs.
* Linux: Identify unnamed native interactive controls by the description of an unnamed, sole-child panel or filler wrapper when the control has no description or accessible label. Reuse walked parent fields without extra parent calls, skip web documents, and preserve uncertainty when wrapper reads fail.
* Restore element bounds and ref clicks for local-position dialogs with different native titles by matching complete sibling-frame inventories with size-identity proofs.
* Keep Linux key focus checks on the command deadline while bounding X11 activation checks to 300 ms, and preserve the last valid window listing when a focus re-read fails.
* Prevent Linux window commands on a stale dialog from targeting a same-size sibling window.
* Refuse Linux key input to dialogs with stale ACTIVE state unless X11 geometry identifies the dialog as active or a same-process transient window has identity proof and a unique listed size.
* Focus Linux dialogs with different accessibility and native titles through coherent frame geometry, while refusing unproved or competing matches and preserving the named-window focus fast path.
* Use coherent Linux window matching for capture, hidden-window filtering, and focus candidates so geometry cannot claim a named frame's client or share one client between competing frames.
* Avoid extra Linux window re-read waits for persistent frame identity mismatches; re-read only when an unclaimed native window indicates a window is mapping.
* **linux:** Window move and resize error details omit unrequested fields from `requested` instead of returning them as `null`; `observed` and `display_bounds` are unchanged.
* Linux: type into the uniquely focused writable paragraph or heading through a document ref, report the actual input target, reuse stable refs across edits, and find document content with `wait --text` or `find --text`.
* Ref identity: document, paragraph, heading, and webarea values no longer count as identity, including on macOS (AXWebArea and AXHeading), Windows (UIA Document), and the web surface. Without a stable name, description, native ID, or retained element token, identity is Unknown; editable content alone cannot identify a ref. Windows behavior is untested.
* Return fully observed `find` matches without requiring an identity or geometry anchor for an unnecessary second read, while preserving saved-ref safety checks.
* `find --surface menubar` no longer returns a `TIMEOUT` that more time cannot fix when part of the menubar cannot be read, such as a menubar Help menu's search field after a menu was opened: matches seen are listed with `complete: false`, a `--first` match with every node before it read is still answered, and no match or a pick the unread part could change (`--nth`, `--last`, `--count`, or a `--first` match after the unread part) returns non-retryable `ELEMENT_NOT_FOUND` with `details.kind: "locator_unreadable_part"`.
* An empty `find --surface menubar` answer carries `data.hint`, which says that closed menus are searched and that a pane or a window that is not open belongs to a window search.
* Treat a macOS window its app ordered out (off screen, gone from AXWindows, on no Space) as closed so post-click waits follow the new window, while windows on another Space, behind a full-screen Space or mid full-screen transition stay present, and bound the Space read by the command deadline.
* macOS: a new table or outline row selection can verify an unconfirmed `AXConfirm`, selection step, or dispatched headed click. Selection alone won't confirm an uncertain `AXOpen` or `AXPress`.
* macOS: a page scroll refused with `kAXErrorCannotComplete` is reported `not_delivered` and safe to retry only when the scroll area is also at its end that way, by its scroll bar value or, without a bar, a content size that fits; unchanged signals of an area with room left keep the uncertain disposition, so a retry cannot scroll twice past a pinned header or a late animation.
* macOS: the `scroll_step_overshoot` message says the last scroll step that moved the target carried it past the viewport edge, instead of claiming every page-sized step did.
* On macOS, clamped or ignored moves and resizes now report the requested and observed geometry. Maximize can still time out, as can move or resize if the deadline expires before any frame is readable and before the readback budget ends.
* Keep time in reserve to close pointer-opened combo popups after choice-read or commit timeouts, preserving the original error and opener delivery disposition. Report an already-hidden Qt combo popup as closed without Escape.
* Document `popup_closed: null` for unreadable state, failed Escape and a Qt pre-Escape recheck that can no longer identify or read the popup, and `popup_close_attempted` to distinguish whether Escape was sent.
* Give stale refs from crashed owners the same process-exit recovery suggestion as refs from exited owners.
* Document the early `popup_cleanup_budget_insufficient` refusal for pointer-only combos and Qt combo rows, and suggest increasing `--timeout-ms` to leave their respective 1000 ms and 500 ms cleanup reserves.
* Report `popup_closed: true` without sending Escape when the Qt combo-row pre-Escape recheck finds the popup already hidden, instead of `null`.
* Restore fresh-snapshot recovery hints in safe stale-ref error envelopes without making complete-absence refusals retryable.
* Guide safe stale-ref recovery through an app-state check when the owner exited or crashed, including platform owner-exit refusals that cannot be retried as they are.
* Attach fresh-snapshot recovery to destroyed Linux targets, stamp exited owners directly on Linux and Windows, and settle complete owner-exit evidence without changing wait retryability.
* Keep non-retryable partial absence errors without snapshot-only recovery.
* Linux snapshots use an explicit AT-SPI composite label for its sole editable child, so distinct fields can be edited by ref.
* **linux:** items of a closed menubar menu are no longer in a window snapshot, a window `find` or a window `wait --text`/selector, and a ref captured earlier for such an item now gets `STALE_REF`; the menubar's top-level openers stay, so open the menu first, then read it with `snapshot --surface menu`
* A web connection from a `launch --cdp` registry entry refuses a WebSocket URL whose path does not start with `/devtools/browser/`, with error kind `cdp_endpoint_not_browser`; before, a page endpoint passed both the handshake and `Browser.setDownloadBehavior` and was used as the browser.

### Performance

* Linux reads contents-index scrollbar evidence alongside labels and values, and skips it for non-index reads.
* Linux contents-index row visibility ignores headers and scrollbars and does not infer list overflow from clipping by an outer scrolling page.
* Contents-index lists inherit scroll-area evidence only when the area contains exactly one list descendant, so a scrolling page does not mark several loaded lists partial.
* Compact snapshots preserve a list's own scroll evidence and inherit collapsed-wrapper evidence through at most three wrappers.
* macOS reads a vertical scrollbar's enabled state for contents evidence only during index-building snapshots, not searches or live-state reads.
* Linux: the composite-label read for unnamed editable fields reads each parent once for all of its fields read together and stops reading siblings at the second editable child, so a form of unnamed inputs no longer costs one full sibling scan per field.
* Linux: wrapper-description fallback reuses the composite-label parent's identity and child list, avoiding duplicate parent lookups and skipping wrapper reads for parents with multiple children.
* Linux: while `launch` waits on a new windowless descendant, such as a tray or background process, it checks the root's children before a full window read, reuses a windowless read while those children stay the same, and remembers a process instance's window once seen.
* Stop retrying Linux menu snapshots after a complete AT-SPI read accounts for a settled mapped popup, and reuse one X11 connection across opening probes unless a failed read requires reconnection.
* **linux:** a window snapshot no longer reads the items of closed menubar menus: LibreOffice Writer `snapshot -i` drops from 216 KB, 1,623 refs (1,502 ambiguous) and 810 ms to 27 KB, 202 refs (101 ambiguous) and 156 ms (median of 7)
* **linux:** Skip the stability sampler for pointer actions on unmoved GTK 4 (window-coordinate) controls on X11 by keeping the snapshot's screen bounds hash with the ref.
* Speed up macOS ref actions on large pages by checking for an on-screen menu window before searching accessibility descendants for a context menu.
* A web ref click sends two fewer requests. Attaching to the page no longer sends `Runtime.runIfWaitingForDebugger` when Chrome's `Target.attachedToTarget` event says the page is running, which Chromium always reports for an attach; a page reported paused, or one with no such event, is still released. The live read before each ref action also proves the page's document once instead of twice, so one `Page.getFrameTree` round trip goes. In headless Chrome 156 on the local VM a web ref click went from a median of 4.1 ms to 3.9 ms (three rounds of 7 runs, warm-up dropped).
* Connecting to a browser launched with `launch --cdp` sends one request fewer: the registry already holds the browser's own WebSocket URL, so the `Browser.getVersion` probe is dropped and `Browser.setDownloadBehavior` proves the connection; a connection also asks `Browser.getVersion` at most once, so `open-url` no longer repeats it. In headless Chrome 156 on the local VM the whole connection took a median of 0.6 ms before and after, so the cut saves one browser round trip, which matters when the browser is slow to answer.
* A web snapshot reads its iframes concurrently: requests to every frame and iframe target are pipelined on the one DevTools connection, at most 16 awaiting a reply, under the same deadline and per-frame budget, with output and ref order unchanged; on the Cambridge Dictionary serendipity page in headless Chrome 156 (25 iframe targets) a snapshot took a median of 93 ms instead of 144 ms (7 runs).

### Documentation

* The interaction skill states what a followed `--wait-for` pays per poll to read the process's open menu when nothing else matched: about 16 ms on a small Windows window, 43 ms on an Edge window, about 23 ms on a small GTK3 window. ([#222](https://github.com/gshost1/agent-desktop/pull/222))
* Clarify that snapshots of non-window surfaces also carry the contents index when their trees have at least 80 nodes.
* The interaction reference says that a second running copy of the same app never satisfies a post-action `--wait-for`, and that the 5 s `wait_owner_unresponsive` check still applies after the ref's window closed.
* The web skill says that `snapshot -i` lists only interactive nodes, so a page's sentences, list items and results are read or counted from the snapshot without `-i` or with `find --role statictext --count`; the reference adds that a text count also includes headings and labels that repeat the word.
* Align the interaction skill, recovery reference and Linux dead-owner test contract with exited and crashed owners and platform owner-exit refusals.

### Testing

* The Windows live E2E suite runs nightly and by hand on a hosted `windows-latest` runner through `scripts/run-windows-e2e-ci.ps1`, with failures reported as failed runs. The new `windows-e2e` workflow also runs non-gating on pull requests that edit it and is never a pull-request gate. Its Preconditions scenario counts a `find` match that carries no ref, legs that need foreground off the fixture steal it onto a window the harness owns, a failed leg's verdict line names the error envelope it saw, the shell-surface legs skip only on evidence that the runner's shell lacks the surface, and an aborted run prints the legs recorded so far. ([#210](https://github.com/gshost1/agent-desktop/pull/210))
* The Linux live E2E text legs restore the document with `set-value` instead of `clear` and a `--headed type`, so a document that reopened with a multi-line draft from an earlier leg is restored instead of failing `INVALID_ARGS` on its newlines (`text_entry.typed_text_reaches_document`). ([#215](https://github.com/gshost1/agent-desktop/pull/215))
* Unit tests pin the edit write dispositions (a refused reservation sends nothing, a failed or unanswered `EM_SETSEL` is `not_delivered` and sends no text, an unanswered `EM_REPLACESEL` is `delivered_unverified`) and the inline-or-worker route. Live Win32 fixture tests cover a held edit thread's late delivery, the pending-write bound, all 8 slots returning when the edit's thread exits without reading the queued messages, a waiting caller that still answers messages sent to its own windows, and disabled and over-limit edits keeping `ValuePattern`. ([#225](https://github.com/gshost1/agent-desktop/pull/225))
* Restore the Linux E2E capture leg's original document before stopping the editor, including on failures, so its draft does not contaminate later legs.
* Require a uniquely named Calculator button rather than an anonymous, ambiguous container before checking Linux E2E dead-owner ref recovery.
* Read component extents only once per node in each zero-bounds observation, while refreshing geometry for the after and restored reads.
* Read raw AT-SPI component position and size separately so Qt window-relative extents cannot inflate the Linux gesture oracle's click target.
* Read the Linux E2E keyboard-combo leg's document by its text role instead of the longest text in the app, and restore that document even when the leg fails.
* Preserve a keyboard-combo failure when restoration also fails, recording cleanup errors alongside it; restoration errors still fail an otherwise successful leg.
* Follow native AT-SPI LabelledBy relations when checking the visible menu item after a Linux selector wait, including GTK4 labels exposed only through Text.
* Give GTK3 fixture containers distinct accessible names so the ref matrix exercises their unsupported-action contract instead of accepting a captured-twin refusal as a pass.
* Name the Qt combo's popup list and toolbar overflow control so hidden-row refusals reach the action contract instead of stopping at captured-twin identity checks.
* Report the expected Qt toolbar overflow object by name when the fixture cannot find it.
* Exercise headed scrolling and headless policy refusal on a native GTK4 fixture's sole scroll area, preserving viewport, checkbox and desktop restoration checks; retain Settings captured-twin refusal coverage in both modes.
* Check only supplied refusal counters, rely on disposition and independent state for identity refusals, and restore Settings scrolling after failures.
* Preserve the original Settings refusal failure when scroll restoration or its readback also fails; report cleanup errors separately and fail otherwise successful legs on cleanup errors.
* Check Linux E2E missing-choice selection as select.absent_choice_refused against its current policy refusal and headed search error, including delivery dispositions, popup cleanup, and an unchanged setting.
* Verify headed Linux E2E Screen Blank selection through delivered_verified and its setting change, classify headless pointer-policy refusal in a separate asserted-refusal leg, and restore the original value even on failures.
* Require `umockdev-run` only for the mock-mouse fixture, reporting affected legs unavailable without blocking other legs or using host hardware.
* Supply a virtual udev mouse to the Linux Settings Mouse fixture so pointer-speed checks do not depend on host input hardware, retaining the real Settings, GSettings, and reopen assertions.
* Install the video plugin and Mutter and GNOME Shell schemas required by GNOME Settings panels in Linux live E2E so fixture crashes do not masquerade as oracle timeouts.
* Open a uniquely named private empty file in a standalone Text Editor instance for semantic typing, wait for its raw window or tab name and empty document before allocating its ref, and remove private session and draft state after stopping the app so later legs cannot restore the deleted file.
* Check the documented unsafe unresolved Terminal handoff instead of claiming successful attribution to a different executable, with independent native-window evidence and exact-owner cleanup.
* Preserve the original handoff assertion when cleanup fails, and report cleanup failures after a successful leg body.
* Resolve owned Linux E2E applications by PID in list-apps before menu waits, rather than passing toolkit labels or launcher names that are not product application names.
* Allow only the identified listener_hold holder from crates/linux/src/system/bus/listener_hold.rs up to 1.5 seconds from process start during Qt cleanup; reject other extra processes immediately.
* Give Linux desktop warm-up a separate 45-second app-mapping budget and include the failing app's last 2048 stderr bytes in tier-unavailable reports.

## [1.5.0](https://github.com/gshost1/agent-desktop/compare/v1.4.0...v1.5.0) (2026-11-15)

### Features

* `snapshot --since <snapshot_id>` takes a normal full snapshot but prints only what changed since that earlier snapshot of the same window and session: `added`, `changed` (with the earlier values in `was`) and `removed` rows with fresh refs, the focused element, and an explicit `unchanged` status, so an agent reads one change instead of the whole tree after each action. Rows pair by the identity refs use, a read that was cut short reports `incomplete` instead of claiming no change, and `--lines` prints `+`, `~` and `-` lines. ([#169](https://github.com/gshost1/agent-desktop/pull/169))
* `find --root <list> --scroll [DIRECTION]` finds an element that a lazily loaded list has not built yet in one call: it pages the list headlessly, looks after each page, turns back at the end, and stops at the first match, at both ends, after 200 pages or at the deadline. `data.scroll` reports the pages sent, whether the view moved and why the search stopped; the view stays where the search stopped. It refuses `--count`, `--last`, `--nth` and `--allow-partial`, which need the whole list. Paging is supported on macOS; elsewhere a search that needs a page returns `PLATFORM_NOT_SUPPORTED`. ([#216](https://github.com/gshost1/agent-desktop/pull/216))
* The fork publishes to npm as `deskref`. `npm install -g deskref` installs one executable under two command names, `deskref` and `agent-desktop`, and on Windows the install links both command shims to the native binary. The executable, its JSON output, its skills and the release archive names keep the name `agent-desktop`, and the `agent-desktop` npm package is still upstream's build.
* On macOS, `select @<text field or text area> "<text>"` selects that exact text, case included, and reads the selection back. The text must occur once: a repeated text fails `AMBIGUOUS_TARGET` with `details.kind: text_occurs_more_than_once` and the first ten `details.offsets`, and an absent one fails `ELEMENT_NOT_FOUND` `text_not_found`; both are `not_delivered` and leave the selection as it was. Selecting a paragraph or a phrase used to take a run of arrow and shift-arrow keystrokes, which held-out formatting tasks miscounted by one.

### Bug Fixes

* `list-notifications` gives each entry a `key` over its app, title and body, with a count that tells identical twins apart; `dismiss-notification --key` and `notification-action INDEX ACTION --key` act only on the entry that carries it, so a list that shifted between the listing and the call cannot redirect a dismiss or an action. ([#34](https://github.com/gshost1/agent-desktop/pull/34))
* A web snapshot leaves out iframes that miss their budget instead of failing with `TIMEOUT` (`cdp_timeout`) and losing the page it had already read. Iframe reads stop short of the snapshot deadline, keeping a fifth of the time left (at least 100 ms); an iframe that times out or is reached after the budget is spent joins `unreadable_frames`, and the `suggestion` counts the iframes left out. On an ad-heavy dictionary page, failed snapshots went from 2 of 16 to 1 of 24, and the one left failed before any frame was read. The main frame keeps the full deadline, and a `--root` drill-down on a web ref still returns `TIMEOUT`. ([#139](https://github.com/gshost1/agent-desktop/pull/139))
* On macOS, a counted child read whose responder refused the range one longer than the count, and answered only the exact range, no longer counts as proof of the count. The recount runs, so a list that grew between the two reads is reported incomplete instead of complete. ([#140](https://github.com/gshost1/agent-desktop/pull/140))
* On macOS, a page scroll whose `kAXErrorCannotComplete` came from its messaging timeout keeps `APP_UNRESPONSIVE` with an uncertain disposition instead of `ACTION_NOT_SUPPORTED`, `not_delivered`, retry safe. The app may finish the scroll after the 250 ms settle window, and a bench fixture whose scroll area shows a pinned header first was reported not delivered in 5 of 5 runs although its rows had moved, so a retry scrolled twice. A refusal the app answers in time is still settled; SwiftUI answers one in 0-4 ms. The settle now reads the frames of the area's first four content children and of the first one's first four children, and one unchanged frame without a scroll bar no longer counts as proof. A reveal that stops because page steps kept moving the target past the viewport fails `ACTION_FAILED` with `kind: scroll_step_overshoot` instead of saying the scroll areas moved as far as they can. ([#144](https://github.com/gshost1/agent-desktop/pull/144))
* On macOS, permission reports that overlap while an Automation consent prompt is pending share one probe instead of each starting its own. A caller took the in-flight probe out of its slot while it waited, so a second caller found the slot empty and left another thread blocked on the prompt; eight overlapping callers now start one probe. ([#146](https://github.com/gshost1/agent-desktop/pull/146))
* A session trace's budget ledger that is new, empty or written by an older version is now replaced through a temporary file and a rename instead of being rewritten in place. A short write followed by an error, such as a full disk, left a ledger no reader parses, and recording stopped for the rest of the session with "Session trace recording ledger is unreadable". Steady-state events still overwrite the same-length record in place. ([#147](https://github.com/gshost1/agent-desktop/pull/147))
* On macOS, `launch` goes on past an AppKit application record that has no process (pid -1 or 0), as `list-apps` does, instead of retrying until it fails `TIMEOUT`. When no running app matched, any skipped record counted as an incomplete inventory, and a record with no process never resolves while the launch waits. A record skipped for any other reason, which could be the app being launched, still fails the read. ([#149](https://github.com/gshost1/agent-desktop/pull/149))
* On Linux, the pinned `rustix` moves from 1.1.5 to 1.1.6 for the unsoundness advisory RUSTSEC-2026-0486 in its xattr functions, which agent-desktop does not call. The advisory failed the dependency audit on every pull request. ([#150](https://github.com/gshost1/agent-desktop/pull/150))
* **linux:** a command that checks permissions first, such as `launch`, `snapshot`, `find` or any action, now answers an unreachable display as the display refusal (`PLATFORM_NOT_SUPPORTED` with `details.display_unavailable` and the bus error in `details.bus_error`, `not_delivered`) instead of `PERM_DENIED` "Accessibility permission not granted". The permission preflight passes a refusal that the adapter raises while reading permissions through unchanged. `status` and `permissions` still show accessibility `denied` with the display suggestion, and macOS and Windows answers are unchanged. ([#157](https://github.com/gshost1/agent-desktop/pull/157))
* `-w` and `--wait-for-gone` after a ref action on a web ref now read the page over its DevTools endpoint instead of the browser window's native accessibility tree, where Chrome's page reads as incomplete. `press down --ref @field -w 'statictext:Honolulu'` on weather.gov used to time out every time and now returns in about 0.49 s; post-action observation after `delivery_uncertain` reads the page the same way, so browser UI outside it, such as a JavaScript dialog, needs `snapshot --surface window`. The web quickstart now teaches the suggestion-list wait in place of a `sleep`: on a three-run Sonnet weather task the median went from 10 to 8 turns and 23.9 s to 16.1 s, and total `sleep` from 13 s to 3 s. ([#159](https://github.com/gshost1/agent-desktop/pull/159))
* `hover` and `drag` with `--headed` refuse a ref that was not proven unique when it was captured before they take the interaction lease or raise any window, under every `--timeout-ms` including `0`. They now answer with the `AMBIGUOUS_TARGET` or `STALE_REF` refusal that resolution gives (`not_delivered`, no `preparatory_focus`) instead of raising the target window first, and with `--timeout-ms 0` instead of a `TIMEOUT` after the raise. ([#160](https://github.com/gshost1/agent-desktop/pull/160))
* A post-action `--wait-for` on a window-owned ref now follows the action into the windows its process opened after the action, so a selector in a dialog the click opened matches instead of burning the whole budget; once the ref's window closes it reads every window of that process. It ends at once with `APP_NOT_FOUND` (`details.kind: "wait_target_gone"`) when the process exits, and with `APP_UNRESPONSIVE` (`wait_owner_unresponsive`) when its reads have failed for 5 s and the platform's process check confirms it is unresponsive. The `--wait-timeout` default drops from 30000 to 10000 ms. ([#162](https://github.com/gshost1/agent-desktop/pull/162))
* `find` saves a snapshot only when it returns at least one ref. A `find --first` whose only match could not get a ref, and any `find` whose matches are all context matches, used to save an empty snapshot, report its `snapshot_id` and take one of the 128 snapshot slots the store prunes older snapshots to keep; it now reports no `snapshot_id` and saves nothing. ([#163](https://github.com/gshost1/agent-desktop/pull/163))
* A command whose deadline ran out before it ever found a state lock held (the ref store lock, the trace budget lock, a session lock, or the interaction lease's lock file on Linux, macOS and Windows) now fails `TIMEOUT` with `details.kind: deadline`, `timeout_ms` and `elapsed_ms`. It used to report `lock_timeout` with `contention_count: 0`, which sent agents looking for another agent holding a lock nobody held. A wait that found the lock held still reports `lock_timeout` with its `purpose`, `path` and count. ([#164](https://github.com/gshost1/agent-desktop/pull/164))
* **linux:** a `--headed` ref `click`, `right-click` or `type` whose aim point cannot be proven inside the target's visible area now falls back to the element's semantic action, as the skill documents for every `not_delivered` pointer refusal, instead of failing with `STALE_REF` `pointer_target_not_visible`; in the Linux held-out bench this refusal cost 3 of 5 LibreOffice strike-through runs 3 to 7 extra turns each ([#165](https://github.com/gshost1/agent-desktop/pull/165))
* An error envelope now also prints one line on stderr, `agent-desktop: <CODE>: <message> -- <suggestion>`, and a successful `batch` prints one per failed entry, so an agent that pipes output into `jq`, `grep` or `head` still sees why the command failed instead of `jq: Cannot iterate over null` (10-20 such hidden errors per task in Linux bench transcripts). Stdout and exit codes are unchanged; `AGENT_DESKTOP_STDERR_SUMMARY=0` turns the line off. ([#168](https://github.com/gshost1/agent-desktop/pull/168))
* `find --first --allow-partial` keeps the match it found outside list and table rows when the follow-up pass that reads the rows cannot finish, and reports it with `complete: true`, the same answer `find --first` gives without the flag. It used to return that pass's own incomplete observation, which could have no match at all. ([#173](https://github.com/gshost1/agent-desktop/pull/173))
* A headless ref action on a ref whose snapshot bounds were measured inside its window (`bounds_space: "window"`, for example GTK4 rows on Linux) no longer fails the `stable` check with "bounds changed since the previous observation": the pre-dispatch wait compares two live screen reads instead of the window-space snapshot hash, so a refusal names only its real reason, such as `supported_action`. ([#174](https://github.com/gshost1/agent-desktop/pull/174))
* Under `--trace-strict`, a command that sends input and whose closing trace record fails after its own result left delivery `unknown` (a mutation that timed out, an internal error) now fails with `delivery_uncertain` and an unsafe retry. When the recording was full, the trace error's `INVALID_ARGS` code was read as an argument refusal raised before anything ran, and the envelope said `not_delivered` with a safe retry, so an agent could repeat an action that may have landed. A delivery the command reported, and an `INVALID_ARGS` refusal raised before any focus or scroll, keep their disposition. ([#175](https://github.com/gshost1/agent-desktop/pull/175))
* `get` and `is` now resolve the ref and read the element under one 5 s budget. Before, the read started a second 5 s budget after the resolution, so a slow window could hold either command for up to 10 s; a read after a 4 s resolution now gets at most the 1 s left. ([#176](https://github.com/gshost1/agent-desktop/pull/176))
* **linux:** `launch` now waits up to `--timeout` for an app that is still starting, where it used to return after 5 s with no window and a suggestion that blamed the app's toolkit. A GTK3 app that opens its window 8 s after the spawn now returns with that window at 8.4 s; it used to return `ok` with no window at 5.5 s. The launch still ends early when the process, or one it started, maps a window on X11 or Xwayland that stays off the accessibility bus for 2 s after the first 5 s (about 7 s), or runs with `NO_AT_BRIDGE=1` or `GTK_A11Y=none` (5 s), and the suggestion names which case it saw. A process that shows no window by the timeout is reported with `agent-desktop wait --event window-opened --app '<app>' --timeout 30000` as the next step. A forwarded launch of a running single-instance app that only changes its page (`launch gnome-control-center --arg datetime --activate`) now returns that instance and its window in 5.5 to 9.0 s, after the same 5 s a forwarded launch waits for a new window, with a suggestion saying it was taken as the owner and the effect was not verified; it used to fail with `ACTION_FAILED` after 30 s. The instance is taken only when it was the one instance on the bus, was started with operands that begin the request's, and serves forwarded launches over the session bus. ([#177](https://github.com/gshost1/agent-desktop/pull/177))
* A ref action on its default budget whose target stays disabled now ends after 2 s with `ACTION_FAILED`, `details.kind: "target_disabled"` and a suggestion that names the element as disabled. Before, it waited the whole 5 s and returned a `TIMEOUT` that called the application busy or unresponsive. Measured on GTK3 and GTK4, it now ends at 2.05-2.21 s instead of 5.02 s, and a control the app enables within about 1.6 s is still clicked. A `--timeout-ms` other than the default waits the whole budget, and `type` gets the same cut on its own default budget. ([#178](https://github.com/gshost1/agent-desktop/pull/178))
* A headless click, right-click, double-click, triple-click or keyboard input refused because the element has no semantic action now says that `--headed` is the way to act on it if the user allows physical input, instead of "Waiting will not help". ([#178](https://github.com/gshost1/agent-desktop/pull/178))
* A ref refused with `AMBIGUOUS_TARGET` `identity_not_unique` no longer blames a refmap from an earlier version and asks for a new snapshot, which refuses the same twin again. The suggestion now says what the capture saw. A menu entry from a whole-window snapshot is pointed at its open menu's `snapshot --surface menu`, a menu-bar title at the keyboard, and a twin inside the open menu at the keyboard or `--headed mouse-click --xy`. ([#178](https://github.com/gshost1/agent-desktop/pull/178))
* `wait --text` searches as deep as `find --text`: 50 levels, up from the snapshot default of 10. Text nested deeper than 10 levels made the wait time out while `find --text` found it at once. One example is a GNOME Text Editor document body once the window title stops showing new words; its text view is 22 levels down. The snapshot the wait saves now covers the same depth. ([#181](https://github.com/gshost1/agent-desktop/pull/181))
* **linux:** `press`, `--headed type` and other key input no longer crash GTK 3 applications and Chrome: the accessibility event listener each agent-desktop process registers is now held by one forked child for 1 s after it sent keys (X11, Wayland, and a call stopped by a signal), instead of leaving while the application's at-spi2-atk key snooper is still answering them; `--headed press --window-id` into a GTK 3.24 `GtkEntry` crashed 3 of 8 times before and 0 of 12 after, Chrome 157 died 5 of 8 times before and 0 of 10 after, with no added command latency ([#182](https://github.com/gshost1/agent-desktop/pull/182))
* On Windows, headed `type` and headed `press --ref` give the target keyboard focus with UI Automation `SetFocus` when it lacks it, then send the keystrokes; a target that refuses focus fails `ACTION_FAILED` "Target did not take keyboard focus" (or `STALE_REF` when it is gone) with nothing typed. Without `--headed`, `press --ref` keeps the plain focus check and moves no focus. Before, raising the window left focus on another control, so `--headed type` into a field that was not already focused failed `ACTION_FAILED` "Target element lost focus before physical input delivery" every time, on the WinForms and WPF fixtures alike, and needed a separate `focus` call first. The result carries a `UIElement.SetFocus` step before `SendInput.type_text` when focus moved. ([#183](https://github.com/gshost1/agent-desktop/pull/183))
* On macOS, `select` on a text field no longer succeeds when the field reads back a different selection: it fails `ACTION_FAILED` with `details.kind: selection_not_observed`, the requested and observed ranges, and `retry: unsafe`, and only a selection that cannot be read back succeeds as `delivered_unverified`. A value that cannot be read, or is too long to read whole, now fails before the write with its own code (`PERM_DENIED`, `STALE_REF`, `APP_UNRESPONSIVE` or `ACTION_FAILED`, `details.kind` `text_value_unreadable` or `text_value_truncated`) instead of `ELEMENT_NOT_FOUND`. ([#190](https://github.com/gshost1/agent-desktop/pull/190))
* On Windows, `press --app` and `press --window-id` deliver to an application hosted in an `ApplicationFrameHost` frame, such as Settings, when its frame is in front. `list-apps` now names such a frame's hosted application under its own pid, as `list-windows` already did, instead of `ApplicationFrameHost.exe`, so `press --app SystemSettings.exe` finds it; and the focus gates read the foreground and keyboard-focus windows under the same identity, so a press to a foreground hosted application no longer fails `ACTION_FAILED` as not in front. ([#202](https://github.com/gshost1/agent-desktop/pull/202))
* On Windows, a headed ref action (`--headed click`, `focus`, `hover` and the rest) on a ref from `snapshot --surface taskbar` or `--surface start-menu` no longer fails `WINDOW_NOT_FOUND` before it acts. The headed focus step looked the ref's window up in the agent window inventory, which leaves the shell chrome out on purpose. It now accepts the window the shell-surface resolver presents under the same id and process. It raises the taskbar, raises a child window id such as the tray toolbar through its own top-level window, and treats a presented Start or Action Center as already in front. `system-tray`, `system-tray-overflow` and `action-center` refs take the same path, but the hosted Windows Server 2025 runner presents none of them, so only the taskbar, Start and child-window paths are measured. ([#203](https://github.com/gshost1/agent-desktop/pull/203))
* On Windows, headed window activation no longer raises a cloaked window, such as a dismissed Start. It fails `WINDOW_NOT_FOUND` before any write, where it used to hand the foreground to a window that is not shown and report success. ([#203](https://github.com/gshost1/agent-desktop/pull/203))
* On Windows, an open WinUI 3 menu flyout, such as the File menu of the Windows 11 Store Notepad, now counts as an open menu for `wait --menu`, `snapshot --surface menu` and the menu checks around actions. The flyout shows in a visible `Microsoft.UI.Content.PopupWindowSiteBridge` popup window that the classic menu-mode and tool-window checks never read, so `wait --menu` timed out and `snapshot --surface menu` failed `WINDOW_NOT_FOUND` while the menu was open. The skill's Notepad save recipe now finds `Save as` in both Notepad builds with `find --surface menu`; on Windows 11 its menu steps reach the Save as dialog, but in 6 of 7 runs that dialog did not save to the written path, and the recipe now says so. ([#204](https://github.com/gshost1/agent-desktop/pull/204))
* `wait --menu-closed` no longer returns before a menu that its opener has not shown yet: on every platform a closed read counts only after the menu was seen open (two closed reads in a row) or after the platform's opener grace (300 ms on macOS, where a menu reads open 5 to 225 ms after a raw click on its opener; 1.5 s on Linux and Windows), a timeout that ends inside the grace with every read closed returns closed, and in `batch` the entry before the wait reads the menu first so a menu-item click ends the wait after two closed reads with no hold. On Linux this replaces the 2 s per-process action clock that raw `mouse-click` and `drag` never wrote. ([#205](https://github.com/gshost1/agent-desktop/pull/205))
* `status` reports an empty `supported_surfaces` again when the platform answers with no surfaces, such as a Linux session whose accessibility bus did not answer, instead of `["web"]` for a web surface that fails `PERM_DENIED` there ([#206](https://github.com/gshost1/agent-desktop/pull/206))
* Linux commands under a display that stopped answering return within their timeout plus one second again when a killed input call left keys or buttons held: the permission preflight no longer spends a detached second trying to release them on a display that did not answer its probe (`snapshot --timeout-ms 2000` 3,530 ms → 2,520 ms, budget 3,000 ms) ([#207](https://github.com/gshost1/agent-desktop/pull/207))
* Linux `select` on a dropdown it had to open no longer answers a missing or refused choice as untouched when the open may have left a change: a libadwaita combo row clicked under `--headed` answers `delivered_unverified` with `retry: unsafe`, because the click focused and selected the row, and a GTK4 `DropDown` it expanded stays `not_delivered` with `details.preparatory_expand: "expanded_then_collapsed"` only when a read shows the popup collapsed again, and is `delivered_unverified` when the collapse fails or cannot be confirmed; both errors carry `details.popup_closed` ([#208](https://github.com/gshost1/agent-desktop/pull/208))
* A post-action `--wait-for` that follows a ref now also reads the menu the ref's process has open, once per poll, so a click that opens a menu matches an item in it; macOS menus stay application-owned and are not read this way. ([#214](https://github.com/gshost1/agent-desktop/pull/214))
* Windows: `snapshot --surface sheet` and the followed post-action wait read the named window's own modal sheet, or the modal dialog that window owns, whether or not the app is in the foreground, and answer `ELEMENT_NOT_FOUND` with `details.kind: "surface_absent"` instead of `WINDOW_NOT_FOUND` when the window presents none. ([#214](https://github.com/gshost1/agent-desktop/pull/214))
* Linux: a snapshot no longer lists `Click` on a GTK4 entry or password field whose only click verb is `activate`, or `Select` on a GTK4 list box, its rows, a tab list or its tabs (or an index-only list of an application whose toolkit does not name itself), because headless `click` and `select` refuse them (`POLICY_DENIED` and `selection_index_unproven`); `--headed click` and `--headed select` still reach them through the pointer. On the GTK4 widget zoo the 9 advertised-but-refused rows of the live ref-contract leg now pass. ([#217](https://github.com/gshost1/agent-desktop/pull/217))
* On Windows, `set-value` and `clear` on a classic Win32 edit, or on a combo box that has an edit, now replace the text the way typing does (`EM_SETSEL` then `EM_REPLACESEL`, step `EditControl.ReplaceSel`), so the application is told the text changed. `ValuePattern.SetValue` arrives as `WM_SETTEXT`, which a multiline edit and a combo box's edit never report to their owner: the Windows 11 Save as dialog's File name field read back the written path while Save wrote the dialog's suggested name. On the hosted windows-11-arm runner the Store Notepad save recipe now saves to the written path 7 of 7 runs, against 0 of 7 before, and a WinForms multiline `TextBox` and editable `ComboBox` now raise `TextChanged`. Read-only and disabled edits, edits that cannot scroll their text (single-line without `ES_AUTOHSCROLL`, multiline without `ES_AUTOVSCROLL`, where `EM_REPLACESEL` like typing would cut a value wider than the box), drop-down lists, values over the edit's text limit or holding U+0000, and every other control keep `ValuePattern.SetValue`. ([#218](https://github.com/gshost1/agent-desktop/pull/218))
* On Windows, `list-surfaces`, `close-app` and the unresponsive-app check now read an `ApplicationFrameHost`-hosted app, such as Settings, under the identity `list-windows` reports. `list-surfaces --app SystemSettings.exe` listed no surface and now lists the frame as `window` and `focused`. `close-app SystemSettings.exe` ended in `TIMEOUT` with Settings still running and now closes it through its frame (`delivered_verified`). The hang check now reads the hosted `CoreWindow`, whose thread is the app's, not the frame, whose thread is the frame host's. The frame host's title bar and other hosted apps' frames stay excluded. ([#221](https://github.com/gshost1/agent-desktop/pull/221))
* macOS: `snapshot --app`, `find --app` and the other `--app` commands on an app in the background with two or more windows read the window macOS names the app's main window, reported in `data.window`, instead of failing with `AMBIGUOUS_TARGET`
* macOS `clear` written through the accessibility API (always headless) on a plain text field or text area now reports `delivered_unverified` with `application_commit: "unobserved"` and the same `commit_hint` as `set-value`, instead of `delivered_verified` while the app's model, which takes the text only when editing ends, still held the old value; a headed keyboard clear is judged by its read-back as before.
* `snapshot --compact` now collapses empty single-child `group` wrappers on every adapter that reports bounds. It checked each wrapper before hiding its bounds and before `-i` pruned its children, so on macOS, Windows and X11 no wrapper collapsed and `--compact` changed almost nothing; on the macOS fixture app bare `group` lines drop from 5 to 1. Refs and their allocation order are unchanged. The FFI `ad_get_tree_exact` compact path had the same defect.
* `session end` and `cursor-overlay disable` on macOS no longer fail with `INTERNAL` ("did not acknowledge the control") when the session's cursor overlay exits before acknowledging the stop; an overlay that is gone counts as stopped, and one still running at the deadline fails with `TIMEOUT`
* On macOS, headless `right-click`, `wait --menu` and `wait --menu-closed` work in a window too large for the accessibility menu search (2,048 nodes). In an AppKit window with 3,000 buttons the context menu opened, but `right-click` failed `ACTION_FAILED` `delivered_unverified` and both waits failed `APP_UNRESPONSIVE` `surface_search_limit`, so whether a menu was open was unknown (2 of 2). The right-click settle now takes its baseline from the window server when the search runs out of budget, and verifies the right-click by the pop-up menu window that appears; the waits answer from the window server's pop-up menu level first, by the same rule the menu gate already used. All three now pass 2 of 2, with the menu window count read independently. Windows small enough for the search behave as before (SwiftUI fixture, a 1,200-button SwiftUI window, Font Book).
* `find --name` keeps the matches whose whole name equals `--name` when it cuts its matches to `--limit`, including the default of 50. The cut took the first 50 matches in traversal order before whole-name matches were moved to the front, so a whole-name match that came after 50 longer names, such as `Save` after a list of `Save As ...` items, was dropped unless the caller passed `--limit 0` or `--exact`. Without a whole-name match the limit keeps the same matches as before.
* `find --help` says that `--text` also matches the ancestors whose subtree holds the text, as the skills and docs already did.
* `find --first --name` returns the first match whose whole name equals `--name`, and the first match only when none does, as the full match list already ordered them. It returned the first match in traversal order, so after a Subscript command `find --surface menubar --name Subscript --first` returned TextEdit's `Undo Subscript` and an agent that acted on it undid its own format. On macOS a match outside list and table rows still comes before any inside them, so `--first` reads no more of the app than before. A read that runs out of budget answers `--first` when it did before; when a whole-name match could still lie in the unread part, the answer is the best match seen and `data` carries `complete: false` and `truncated: true`, even without `--allow-partial`. `--last` and `--nth` keep traversal order, and the C find entry point is unchanged.
* macOS `find --scroll` now puts the scroll bar back when the measured page write after its probe step fails, and ends the page with a `delivered_unverified` error (`details.kind: scroll_search_probe_not_restored` when the restore was not taken) when the bar cannot be put back, instead of reading the 0.2% probe shift as a moved page and walking a refusing list to 200 pages or both ends without loading it.
* `find --scroll` now counts a page sent at an end of the area that read unchanged as `data.scroll.pages_at_end`, and any error after such a page is `delivered_unverified` instead of retry-safe; on macOS a page whose settle reads are cut short by the deadline is `unconfirmed` instead of an end.
* `find --help` now says `--timeout-ms` defaults to 20000 with `--scroll`, as the skill and docs already did.
* On macOS, a `launch` that reports no window while the app holds one off the current Space now says so in `data.suggestion`. Behind a full-screen app, `launch Calculator` answered `ok` after about 2 s with no `data.window` and no note, and every later command failed `WINDOW_NOT_FOUND`: macOS gives a window on another Space no accessibility tree, so it cannot be read. The note names the possible causes (another Space, a full-screen app in front, a hidden app, a window the app keeps hidden), says that commands will fail `WINDOW_NOT_FOUND`, and gives the remedy that worked: a fresh `launch --activate` opens the window on the current Space (Calculator behind the fixture and the fixture behind a full-screen AppKit window, 2 of 2 each), while activating an app that is already running does not move its window. The 2 s wait is unchanged, because the public window list cannot tell a window on another Space from one that is still being placed.
* Error suggestions and the macOS references now name the headless route first and offer `--headed` only instead of it, if the user allows physical input, so an agent no longer reaches for physical input as the default retry or runs a menu command twice. An ignored Return on a sheet with no default button now says to click the button that confirms it, not Close or Cancel.
* On macOS, a pointer action on a target covered by another app's window is refused even when accessibility cannot say what is under the point. The hit test answered `unknown` there, which the `receives_events` check accepts, so `hover` reported `delivered_verified` while the pointer sat on the covering window: with a window of an unresponsive process over the target, 2 of 2 on the SwiftUI fixture and Calculator, and the fixture never saw the hover. The hit test now reads the window server's window list at the point when accessibility gives no answer, and a normal, floating or panel window of another process in front of the target's window refuses with `occluder.role: "window"` and its bounds, `not_delivered`. System windows at the Dock level or above, such as the Dock's and the screenshot tool's transparent full-screen windows, and agent-desktop's own cursor windows never count. Uncovered targets and targets covered by a responsive app behave as before.
* macOS: `dismiss-notification` and `dismiss-all-notifications` carry `data.list_restore: "unverified"` when Notification Center could not be closed or focus restored after the dismissal, as `notification-action` already did
* macOS: `clear` on a text element that is already empty writes nothing and returns `not_delivered` with no commit hint, instead of `delivered_unverified` with a hint to end the edit
* `find` lists matches whose whole name equals `--name` before matches whose name only contains it, each group in traversal order. After a Subscript command, TextEdit's Edit menu holds `Undo Subscript`, which came first for `find --surface menubar --name Subscript`; an agent that clicked `matches[0]` undid its own format, and held-out task hn1 failed 5 of 5. `--first`, `--last` and `--nth` keep traversal order.
* On macOS, a floating panel on screen, such as TextEdit's Fonts panel or an inspector, is listed by `list-windows`, and `snapshot`, `find` and ref actions reach it by `--window-id`. The inventory read only normal-level windows, so the panel was missing from `list-windows` and `snapshot --window-id` failed `WINDOW_NOT_FOUND`, and a format the app offers only there, such as strikethrough, could not be applied. A panel-level window is listed only while it is on screen and its process publishes it as an accessibility window.
* On macOS, a headed action on a floating panel, such as a click or `select` in a Fonts or inspector panel, no longer fails `TIMEOUT` "window did not become main". A panel never becomes the main window, and the headed focus check waited for that; a window whose subrole is `AXFloatingWindow` or `AXSystemFloatingWindow` now counts as in front once raised.
* On macOS, `set-value` on a text field gives the field keyboard focus inside its window before it writes, so the app's own edit handling runs. Writing an unfocused AppKit field changed its text without telling the app: on Preview's Adjust Size sheet, height 512 reported verified, the width stayed 600 and the image saved at 600x400. It now saves at 768x512. The app is not brought forward.
* On macOS, a headless `press` of a command chord on a character key, such as `cmd+b`, to an app that is not frontmost and has no enabled menu item for the chord fails `ACTION_FAILED` with `details.kind: command_chord_unrouted_in_background`, `not_delivered` and a safe retry. Such a chord posted to a background app does nothing, and it used to be reported delivered. Navigation chords and plain keys still go to the app.
* On macOS, when `--app` matches several windows and none is the one focused window, `snapshot`, `find` and the other `--app` commands choose among the standard windows and dialogs, passing over floating panels and untitled windows such as a search field's suggestion list. Since `set-value` focuses its field, typing into Font Book's search field opens that list, and `snapshot --app` failed `AMBIGUOUS_TARGET` "More than one window matches the target"; held-out task fn9 then scrolled a 300-row grid by hand. A focused panel is still chosen, and a panel is read with `--window-id`.
* On macOS, `set-value` on a plain text field returns `delivered_unverified` with `details.application_commit: "unobserved"`, `verification_reason: "edit_commit_unobserved"` and a `commit_hint` that says to run `focus` on another field in the same window before Save, instead of `delivered_verified`. Many fields hand their text to the document only when editing ends, so the field read back the new value while Save wrote the old one: an Automator URL field and Automator action fields in held-out runs. A text area, such as Script Editor's text in held-out runs, reports `unobserved` with a hint to check the saved result, since ending the edit commits some text views and not others. A search field, secure field or pop-up button keeps its verified read-back. `set-value` never confirms the field itself, because Return in a field can submit a form, run a search or send a message.
* On macOS, a `set-value` whose focus step fails with a lost permission, a vanished element or an unresponsive app now reports that error, and a focus step whose delivery was uncertain makes a later refusal `delivered_unverified` instead of `not_delivered`.
* On macOS, where a menu and the menu bar belong to the application rather than a window, a post-action `--wait-for` after a menu or menu bar item now reads the process's focused window and the windows the command opened, so `click <menu item> --wait-for ":Width"` matches as soon as the effect shows instead of polling the menu bar until the timeout.
* A post-action `--wait-for` that no window satisfies also reads the sheets, alerts and popovers the process has open, where the platform exposes them (all three on macOS, sheets and alerts on Linux, sheets on Windows), so a selector in a sheet the action opened matches; on macOS Preview, Adjust Size… with `--wait-for ":Fit into"` drops from a 10.5 s timeout to under 1 s.
* macOS `notification-action`, `dismiss-notification` and `dismiss-all-notifications` now keep a delivered result when closing Notification Center or restoring the previously frontmost app fails afterwards, with `details.list_restore: "unverified"` on `notification-action`, instead of replacing it with the cleanup's retry-safe error; a failed operation keeps its own error and names the unverified cleanup, and `list-notifications` still reports its cleanup failure.
* macOS `dismiss-notification` now reports `retry: unsafe` on any error raised after a dismiss strategy was sent or may have been (a failed re-read, a permission preflight, a pointer policy refusal, or all strategies failing), instead of the error's own retry-safe `not_delivered`; `dismiss-all-notifications` does the same for a list read that fails after an earlier dismissal.
* A post-action `--wait-for` that matches a sheet, alert or popover on a window other than the first one of the process now returns that window's overlay instead of timing out while it snapshots the first window, and an overlay without the selector on one window no longer hides one with it on another.
* A post-action `--wait-for` reads overlays only on windows of the ref's own process instance, so a second running copy of the same application can no longer satisfy it.
* A post-action `--wait-for` whose window reads fail with `PERM_DENIED` now ends with `PERM_DENIED` and the action's delivery disposition instead of a `TIMEOUT`, and reads that fail as unresponsive after the ref's window closed now reach the 5 s `wait_owner_unresponsive` check.
* A post-action `--wait-for` after a menu bar item in a background application, where no window is focused, now reads the application's front window, and one taken without a window baseline reads the focused window instead of no window.
* An error whose input may already have landed (`disposition.retry: "unsafe"`) no longer tells the caller to retry. On macOS, a click, right-click or other ref action whose fallback chain sent a step without a confirmed effect now says to take a new snapshot before acting again, instead of naming another mechanism such as `mouse-click --button right`; a second right-click could land in a menu that opened late. Windows `dismiss-notification`, the Linux notification expand, and the Linux restore, focus and window-write refusals sent after the request say to read the state first.
* On Windows, an `open-system-surface` or Action Center close that times out or fails a read after its accelerator, invoke or Escape was sent is now `delivered_unverified` instead of `not_delivered` with `retryable: true` or the read's own disposition; a taskbar surface awaited without input stays `not_delivered`, and so is a close whose re-raise was sent before the surface declined the foreground. On Linux, an open or close of the message list or system menu that times out or fails a read after the toggle click is `delivered_unverified` instead of `unknown` or the read's own `not_delivered`. The surface's opener toggles, so a blind retry could undo a late first press.
* On macOS, `focus-window --app`, `screenshot --app`, `minimize`, `maximize`, `restore`, `move-window` and `resize-window --app`, and `press --headed --app` now choose among the standard windows and dialogs when the app has several windows and none is the focused one, as `snapshot`, `find` and `wait` already did. They failed `AMBIGUOUS_TARGET` while an unfocused app showed a floating panel or a search field's suggestion list beside its document window. Two standard windows still fail `AMBIGUOUS_TARGET`, a focused panel is still chosen, and `--window-id` still names the exact window. Windows and Linux report no auxiliary windows, so their choice is unchanged.
* macOS: `wait --event surface-appeared` no longer fires when focus moves between windows of an app with an open sheet or popover; `list-surfaces` now reads every window and names each sheet, popover and alert by its own window id (`w-<n>`), not by its position under the focused window
* macOS: a surface read that could not see some surface kinds is reported to `wait --event` as incomplete, so it fires neither `surface-appeared` nor `surface-dismissed`; in an app too dense for the menu search, the window server's menu-window list settles that no menu is open
* macOS `select <textfield> <text>` now reads the field's value again after the selection write and verifies only when the text at the selected range is still the requested text, so a field edited between the search and the write fails `ACTION_FAILED` with `details.kind: selection_text_changed` and `retry: unsafe` instead of reporting a verified selection of other words; an unreadable or truncated value leaves it `delivered_unverified`.
* macOS `select <textfield> <text>` now counts overlapping occurrences, so `ana` in `banana` fails `AMBIGUOUS_TARGET` with both offsets instead of selecting the first one as if it were unique.

### Performance

* On macOS, `list-windows --app` and the owner lookup of every `snapshot --app`, web snapshots included, confirm through libproc that each selected process is still the one the first AppKit read named, instead of taking a second AppKit workspace snapshot. On the bench Mac with Chrome, the median `snapshot --app "Google Chrome" --surface web -i --lines` went from 31.0 ms to 26.5 ms and `list-windows --app "Google Chrome"` from 30.9 ms to 27.1 ms. An owner that exited or relaunched still makes the read retry, and a pid libproc cannot read still gets the second snapshot. ([#137](https://github.com/gshost1/agent-desktop/pull/137))
* `skills get desktop` and `skills get platform` now print short guides (8,927 bytes, about 3,700 tokens, together on Linux, down from 28,157 bytes and 11,000 tokens), so the guide an agent loads before its first action no longer dominates every later model call; the detail moved to topics such as `skills get desktop refs-and-retry` and `skills get linux capabilities`, and `skills get <topic>` resolves a core or platform topic with no skill name ([#167](https://github.com/gshost1/agent-desktop/pull/167))
* `snapshot`, `find` and every other command that saves a snapshot no longer sync `refmap.json` and `latest_snapshot_id` to disk on Linux and macOS, which removes the 4 fsyncs every save issued (68-86 ms per command on a loaded Linux ext4 laptop, about 18 ms idle; `snapshot -i` on GNOME Text Editor went from a 135 ms to a 78 ms minimum). Both files are still written to a temporary and renamed into place, so a killed process leaves the old file or the new one whole. After a power loss or kernel crash, a ref map or pointer that is missing, empty, cut short or unparsable now reads as a missing snapshot (`SNAPSHOT_NOT_FOUND`, retry safe after a new snapshot) instead of `INTERNAL`. Windows, session manifests, agent profiles and trace files keep their flushes. ([#170](https://github.com/gshost1/agent-desktop/pull/170))
* Linux `snapshot --surface sheet` and `--surface alert` look for a dialog drawn inside the window with a structural read that skips web page content, and give only the found dialog the full read: on a Chrome page of 3,800 nodes with no dialog the answer comes in 89 ms instead of 1,989 ms, and a libadwaita in-window alert in 60 ms instead of 103 ms; a window that is itself the dialog, such as a GTK3 or Qt file chooser, is still read once ([#209](https://github.com/gshost1/agent-desktop/pull/209))

### Documentation

* The 1.4.0 entry for [#116](https://github.com/gshost1/agent-desktop/pull/116) said `close-app` "now returns within 3 ms" of the app's exit. That bound is not established: the exit was timed by a separate kqueue watcher whose own scheduling is in each number, and on System Settings `close-app` returned before the watcher saw the exit (lags down to -14.8 ms, and -21.8 ms before the change). What was measured is a median exit-to-return lag of 0.8-0.9 ms on Calculator, the SwiftUI fixture and Weather, down from 1.3-12.5 ms, with a largest lag of 2.4 ms over 10 runs per app.
* A pull request now adds its changelog entry as a fragment, `changelog.d/<branch>.md`, instead of editing `CHANGELOG.md`, so parallel pull requests no longer conflict at the top of the changelog. The release pull request runs `python3 scripts/changelog_fragments.py assemble --version <x.y.z>`, which writes the release section with its headings in order and every bullet byte for byte, and deletes the fragments. The pre-commit hook checks staged fragments and refuses a `CHANGELOG.md` edit outside a release, and `scripts/check-release-consistency.sh` checks every fragment and fails while the changelog's top section is not the workspace version. ([#142](https://github.com/gshost1/agent-desktop/pull/142))
* The Linux skill, its references and the Linux and configuration docs pages now say a state root that group or other users can reach fails `PERM_DENIED` with `details.kind: "state_directory_permission"` and the `chmod` that closes it, as the code has answered since #41, instead of `INTERNAL`; a skill test fails if any skill pairs that refusal with `INTERNAL` again. ([#189](https://github.com/gshost1/agent-desktop/pull/189))
* The batch key rule in the system commands reference names the three flags whose JSON key is not the flag name: `--ref` is `ref_id`, `--state` is `states` and `--arg` is `args`. Entries written with the old rule (`ref`, `state`, `arg`) were rejected. A test now parses every `agent-desktop batch` example in the skills. ([#192](https://github.com/gshost1/agent-desktop/pull/192))
* The interaction reference's fallback for opening a context menu, `--headed mouse-click --button right`, now includes the required `--xy X,Y`. Without it the command exits 2. ([#192](https://github.com/gshost1/agent-desktop/pull/192))
* Two Linux skill lines no longer advise a blind retry after input that may have landed: an Overview-blocked `click` or `expand` (`popup_not_observed`) and a shell read that lost a node after the opener was pressed now say to look before pressing again, and `scripts/check-retry-advice.sh` now also fails a skill passage that names a may-be-delivered error kind and advises a retry without a look-first step. ([#211](https://github.com/gshost1/agent-desktop/pull/211))
* The Linux docs page now says a state root that group or other users can reach fails `PERM_DENIED`, not `INTERNAL`, which #189 missed; the test that guards the skills now reads every docs-site page too.
* The macOS platform skill is now a short page of recipes: select text by content, format then save, floating panels, long lists through the search field, waiting with `-w`, linked fields, key chords. The full rules moved to the references `actions` and `apps-and-windows`, read with `agent-desktop skills get platform <topic>`. `skills get desktop` plus `skills get platform` printed about 32 KB on macOS, past the roughly 30,000 characters an agent shell tool shows, so agents saw only a 2 KB preview of the platform skill. They now print about 12 KB.
* The macOS skill no longer sends agents to a window's search field to reach an item in a long list. It says to read the whole snapshot or `grep` it for the item's name, and, after a search, to act on the refs the filtered list shows instead of typing again. It also says `--headed` is only for when the user allows physical input, as the core skill does, and drops its "Rerun with `--headed`" advice for background menu chords: an unrouted chord says to `click` the menu item, and a disabled item in a background app says to run `focus-window` and press again. On held-out task fn9 (Font Book) every candidate run followed the old line into the search field, missed the filtered list's `Show <name>` item, then retyped the text and turned to `--headed` in all three runs, a median 127 s; main, which never searched, often took 20-28 s.

### Testing

* `scripts/cargo-test-isolated-home.sh`, which the pre-commit hook and CI use to run tests, now also cuts tests off from the caller's desktop session: it unsets `WAYLAND_DISPLAY` and `AT_SPI_BUS_ADDRESS`, points the session bus at a socket that does not exist and gives cargo a private `XDG_RUNTIME_DIR`, which it removes when cargo exits. On a machine with a logged-in desktop, a test that reaches for the session bus can no longer land in the user's real session. `AGENT_DESKTOP_TEST_SESSION=inherit` keeps the caller's session. The full Linux host test set passes the same with the session cut (5203 passed, 0 failed). ([#158](https://github.com/gshost1/agent-desktop/pull/158))
* The pointer focus-phase tests run their hover, drag and leased-wait polls on the virtual test clock, so a slow CI runner no longer runs out their 400 ms budget before a transient overlay clears; under a 2% CPU quota the old tests failed 19 of 20 runs and the new ones pass every run. ([#161](https://github.com/gshost1/agent-desktop/pull/161))
* Five tests that failed CI on loaded Windows, Windows ARM64 and macOS runners no longer depend on host speed or on a Windows file-deletion rule: the reaped-session test removes only the session manifest, the cross-session cursor test holds the other session's cursor for the whole command, the continuously moving target gets a 5 s budget (a 40 ms live read per call reproduced the 60 ms TIMEOUT), the vanished partial download gets a 30 s budget against its 1.5 s grace, and the Windows E2E harness self-test pays `powershell.exe`'s cold start before its first case. ([#166](https://github.com/gshost1/agent-desktop/pull/166))
* The FFI test `session_adapter_keeps_exact_ref_missing_generation_fail_closed` holds its own isolated `HOME`: it read a `HOME` that a parallel test deletes when it ends, so session adapter creation failed with `HOME directory not found` about once in 100,000 creations under four concurrent `HOME` swaps, and once on Windows CI. ([#184](https://github.com/gshost1/agent-desktop/pull/184))
* Five merged fixes now have a test that fails when the fix is reverted at the path the command takes, not only at a helper: the window-stack occluder answers an undecided macOS hit test (#145), both launch paths add the off-Space note to a windowless result (#152), `list-surfaces` keeps a sheet listed after focus moves to another window of the app (#155, macOS E2E), a counted child read that fell back to the exact range runs the recount (#140), and a headless command chord that no menu item takes is refused for a background app (#148, macOS safe-semantic E2E). The hit test, launch results and counted child read take their live reads as inputs so the tests can stub them; behavior is unchanged. ([#198](https://github.com/gshost1/agent-desktop/pull/198))
* Linux CI tests run on their private session bus again. Since `scripts/cargo-test-isolated-home.sh` began cutting tests off from the caller's session (#158), the CI step that starts a `dbus-run-session` bus and an Xvfb display for them passed `AGENT_DESKTOP_TEST_SESSION=inherit` to nobody, so every Linux test ran with no D-Bus or AT-SPI. The step now opts in and sets `AGENT_DESKTOP_EXPECT_SESSION_BUS=1` and `AGENT_DESKTOP_EXPECT_ATSPI_BUS=1`, and two new Linux tests fail when a run that promises those buses cannot reach them, so the lane cannot lose its bus again unnoticed. The Linux live E2E workflow now runs on every pull request and every push to `main` on a hosted Ubuntu runner (the x11-openbox tier), instead of only by hand on a runner named by a repository variable that was never set. Until all of its legs pass, a failing run does not fail the workflow; the job still shows red and writes its pass, fail and error counts and each failing leg with its reason to the run summary. ([#201](https://github.com/gshost1/agent-desktop/pull/201))
* Windows live tests that open the fixture's context menu now hold the shell-surface test lock, so a concurrent test that opens Start, Action Center or the Windows 11 Notepad flyout can no longer take the foreground and dismiss the menu mid-test (red `Test (Windows ARM64)` on main 92094e8ea, green on rerun). ([#213](https://github.com/gshost1/agent-desktop/pull/213))
* CI now validates changelog fragments and runs the fragment tool's self-test on every pull request, in the Format job. The fork's Supply Chain workflow does not run, so a malformed fragment used to be caught only at release.
* The hosted macOS E2E lane runs on macos-15 for pull requests and on macos-15 and macos-26 for pushes to main. GitHub runs five macOS jobs at a time on this account, and two E2E jobs per pull request made every parallel pull request wait.
* Add a dispatch-only workflow that builds the release CLI for each requested arm on the hosted Windows runner and measures per-command latency against the Win32 and WPF fixtures.
* `scripts/check-retry-advice.sh`, in the pre-commit hook and CI, fails when an error built as `delivered_unverified` or `uncertain`, or passed through a helper that stamps one, carries a suggestion to retry, rerun or act again without saying to look first.
* The Linux cursor-overlay Wayland refusal test binds its socket in the temp directory instead of the crate directory, so it no longer fails with `path must be shorter than SUN_LEN` in a checkout whose path is longer than about 60 bytes.

### Known issues

* On macOS, a `close-app` step brought another app's window to the front in 1 of 50 benchmark runs: `close-app "Calculator"` returned and ColorSync Utility, the app the task was driving, came to the front 0.3 s later.
* During a Finder rename, `snapshot --surface focused` returns `STALE_REF` (`focused_window_changed`), because the floating rename editor counts as the focused window. The macOS skill's rename recipe, which writes to the editor's ref from that snapshot, cannot be followed on Finder.
* On a slow table of 150 rows, commands returned `WINDOW_NOT_FOUND` twice.

## [1.4.0](https://github.com/gshost1/agent-desktop/compare/v1.3.0...v1.4.0) (2026-11-11)

This fork release publishes GitHub release archives and checksums only. It does not publish to npm.

### BREAKING CHANGES

* `find` match objects, and `AMBIGUOUS_TARGET` candidates from a locator, omit `ref_id`, `name`, `value`, `states` and `path` when absent or empty, as the JSON contract states, instead of printing `null`, `""` or `[]`. Part of envelope 2.7; a reader that indexed those keys must read a missing key as that value. ([#93](https://github.com/gshost1/agent-desktop/pull/93))
* The response envelope moves to 2.7. On Windows and Linux, and on macOS for an element with no address, a ref whose snapshot could not read the window whole now fails `STALE_REF` with `details.kind: identity_unproven` and `details.reason: capture_incomplete` instead of `AMBIGUOUS_TARGET` `identity_not_unique`; retake the snapshot with a larger `--timeout-ms`. A ref whose capture left a recorded field unread, or that no capture judged, fails `STALE_REF` `identity_unproven` with `details.reason: capture_unknown`. `identity_not_unique` now means the capture saw an identical twin. Refmap entries record `capture` (`unique`, `ambiguous`, `incomplete`, `unknown`); refmaps from earlier versions still load. See the version 2.7 migration in `docs/json-output.md`. ([#14](https://github.com/gshost1/agent-desktop/pull/14))

### Bug Fixes

* On macOS, an element of a SwiftUI scroll view whose content is narrower than the view is no longer marked `offscreen`, and a pointer action or `scroll-to` on a target scrolled out of view now brings it back instead of failing. Such a view centres its content but reports a frame that starts right of it, so a visible button was judged offscreen: in a fixture window 1200 or 1430 points wide a plain headed `click` failed every time, and it now passes. A scroll area now clips only on an axis where its `AXContentSize` overflows it. Scroll-into-view scrolled the right way, then asked a SwiftUI scroll view whose frame starts right of its content, and which has no horizontal scroll bar, for `AXScrollLeftByPage`; the refusal failed `click` as `APP_UNRESPONSIVE`. It now scrolls each enclosing scroll area, innermost first, on the axis that hides the target, and stops asking for a direction the area cannot move in. After `scroll-to` pushed the fixture's button off screen, headless and headed `click` went from 0 of 2 to 2 of 2. On an AppKit scroll view, clicks on targets off screen to the right, below and in the far corner went from 0 of 6 to 6 of 6. In System Settings, whose panes are hosted out of process and answer `AXWindow` with no value, `scroll-to` and `hover` went from 0 of 3 to 3 of 3. A page scroll refused with `kAXErrorCannotComplete` now fails `ACTION_NOT_SUPPORTED` with `not_delivered` instead of `APP_UNRESPONSIVE` only when its scroll bar, its content and the content's first child all stayed readable and unchanged for 250 ms; otherwise the scroll counts as done or keeps its uncertain disposition. A `scroll-to` or pointer action whose target ends visible now succeeds even when a scroll step on the way failed. ([#126](https://github.com/gshost1/agent-desktop/pull/126))
* On macOS, a ref action on an element the app has removed fails `STALE_REF` with `details.kind: retained_object_stale` and `reason: dead`, `not_delivered` with a safe retry, so an agent refreshes its snapshot. It used to fail `APP_UNRESPONSIVE` "Accessibility surface read failed for AXRole" with `ax_error: -25202`: the saved element token still resolved, and the first read of the removed element reported it invalid. An invalid app root, or an invalid element met while walking a live window, still fails `APP_UNRESPONSIVE`. ([#123](https://github.com/gshost1/agent-desktop/pull/123))
* On macOS, `find --text`, `--has` and `--has-not` answer on windows that hold a tab group or a radio group. Such a group reports its selected child as its value. The locator read that as a failed read and retried the walk until the deadline, then failed `TIMEOUT` `locator_incomplete`: 0 of 7 on the SwiftUI fixture and Script Editor, now 7 of 7. A value that is an element now reads as no value. Snapshots are unchanged. ([#106](https://github.com/gshost1/agent-desktop/pull/106))
* On macOS, `launch` reports its window once the window server has placed it, so `data.window` carries the real bounds and `visible: true`. It used to report the first record it read, which right after start-up had bounds 0x0 and `visible: false` (Calculator, the SwiftUI fixture, Script Editor and Weather, 16 of 16 launches). `launch` takes the time the window needs to reach the screen, read at most 50 ms apart. ([#100](https://github.com/gshost1/agent-desktop/pull/100))
* On macOS, read the focused application and the focused element correctly on the first read of a command. A fresh process with no window-server connection got `kAXErrorCannotComplete` from every system-wide focus read, so a headed pointer command could not tell whether its target was already in front and always raised it. In fresh processes the classification went from unknown 14 of 14 to already-front 7 of 7 and front-changed 7 of 7. ([#99](https://github.com/gshost1/agent-desktop/pull/99))
* On Linux, a `launch` whose spawned process keeps running without joining the accessibility bus (a plain X11 program, an app started with `NO_AT_BRIDGE=1`, Chromium without its accessibility flags) now returns after 5 s instead of waiting the whole `--timeout` (30 s by default); `--activate` still waits until `--timeout`. The result is still `ok` with the spawned pid and no `window`, and its suggestion says the process did not join the bus. ([#81](https://github.com/gshost1/agent-desktop/pull/81))
* On Linux and Windows, a notification dismiss, clear or action that was delivered keeps its result when closing the message list or Action Center it opened fails afterwards. The close failure used to replace the result with an error, so an agent retried and the retry dismissed or pressed the notification that had moved into the same index. `notification-action` now names the failed close as `details.list_restore: "unverified"`, and so does the error of a mutation that failed when the close failed too. ([#111](https://github.com/gshost1/agent-desktop/pull/111))
* On macOS, a headless `right-click` fallback that selects the row before opening its menu now reports that selection as delivered: a later failure, or no menu, carries `retry: unsafe` instead of `not_delivered` and a safe retry. Each fallback also opens a menu only after the app's menu state reads as closed; a read that stays unreadable, as while an app tracks a menu, no longer lets a second `AXShowMenu` or `AXPress` through, and a pop-up menu window of the app on screen counts as an open menu. In a window too large for the menu search, a step still acts when the window server shows no menu window. ([#86](https://github.com/gshost1/agent-desktop/pull/86))
* On macOS, give the Automation permission probe a 3 s limit. While a consent prompt waits on screen the probe blocked until someone answered it, 175 s in one run, and every command hung with it. Command preflight and the FFI `ad_check_permissions` no longer probe Automation at all; `status` and `permissions` report it as unknown when the probe does not answer, and a notification command that needs it fails with `APP_UNRESPONSIVE` and says a prompt may be waiting. ([#7](https://github.com/gshost1/agent-desktop/pull/7))
* On macOS, verify a headless `click` that opens a file or folder item through `AXOpen` by its effect: the sheet that held the item closes, or the window takes the item's name as its title. Such an open used to be reported as failed when the app answered with an uninformative code or reported the removed target as invalid. ([#9](https://github.com/gshost1/agent-desktop/pull/9))
* Report a directory or file in agent-desktop's state (`AGENT_DESKTOP_HOME`, default `~/.agent-desktop`) that group or other users can reach as `PERM_DENIED` with `details.kind: "state_directory_permission"`, the path in `details.path` and a suggestion naming the `chmod` that closes it (`700` for a directory, `600` for a file), instead of `INTERNAL`. This covers state locks, state files, `session start` and the cursor lock directory. A state lock's other permission refusals now also answer `PERM_DENIED`, as an io permission refusal elsewhere already did, without that advice. ([#41](https://github.com/gshost1/agent-desktop/pull/41))
* Stop a headed `hover` or `drag` from raising its target's window again on every attempt until `--timeout-ms`. A pointer command now raises the window at most once, counting a raise made while the wait scrolls; later steps skip the raise, and a focus call that fails after it may have raised ends the command. A target that stays covered ends with a `TIMEOUT` that carries the last refusal in `details.refusal` (the failed `receives_events` check with its covering element) and a suggestion that says the target was covered, instead of one that blames a busy application. Every error before the input carries `details.preparatory_focus`. ([#58](https://github.com/gshost1/agent-desktop/pull/58))
* A session trace event waits for the shared budget lock as long as its command's deadline allows (up to 2 s, and up to 250 ms from the event when less is left) instead of a fixed 50 ms, and the recording ledger is rewritten in place without a disk sync per line. Eight parallel `--trace-strict` writers in one Linux session failed 128 of 200 commands on lock timeouts and now fail none, and a traced command costs 7.4 ms instead of 21.2 ms. ([#83](https://github.com/gshost1/agent-desktop/pull/83))
* On macOS, `launch` checks on every poll whether the launched app is still running. An app that finishes starting up and then quits, with or without `--activate`, now fails with `APP_UNRESPONSIVE` "Launched application exited before it presented a window" and `details.pid` instead of answering `ok` with the pid of the exited process, which with `--activate` took the whole `--timeout`. An app that restores several windows with none focused now launches with no `data.window` and a `data.suggestion` that lists the window ids, instead of failing with `AMBIGUOUS_TARGET`. ([#88](https://github.com/gshost1/agent-desktop/pull/88))
* On Linux, `launch "<Name>"` where several desktop entries share the name and run the same command (one canonical executable reached under the same name, the same arguments and working directory, as when a package installs an app under two desktop ids such as `com.google.Chrome` and `google-chrome`) now starts the first of them in desktop id order that can start, instead of failing with `AMBIGUOUS_TARGET`. Entries that run different commands still fail with `AMBIGUOUS_TARGET`, now with `details.candidates` (each desktop id with its command or the reason it cannot start) and a suggestion to rerun the launch with a startable desktop id, instead of a suggestion to refresh refs. A running process now answers `--app` to every desktop entry that runs its exact command, and `list-apps` reports the first such desktop id as `bundle_id`. ([#89](https://github.com/gshost1/agent-desktop/pull/89))
* On Linux, a process that `launch` started from a desktop entry now answers `--app` to that entry's name and desktop id, and `list-apps` reports that id as `bundle_id`, even when the entry's command is a wrapper that `exec`s another binary or the launch passed arguments. After `launch google-chrome --cdp`, `--app "Google Chrome"` used to fail with `APP_NOT_FOUND` because the browser runs `/opt/google/chrome/chrome`, which no entry names. The launch records the entry for that exact process instance and executable in a private file under `$XDG_RUNTIME_DIR`. ([#103](https://github.com/gshost1/agent-desktop/pull/103))
* On Linux, a launch's desktop-entry record now names the process by pid as well as instance and executable, and is written only for the process the launch spawned itself (or that a wrapper `exec`ed). A process the launch merely handed off to no longer takes the entry. A launch of a `.desktop` file shadowed by an earlier data directory no longer records the shadowing entry. A recorded entry's `Name` becomes an alias only when no entry running another command shares it, and records of exited processes are dropped. ([#121](https://github.com/gshost1/agent-desktop/pull/121))
* On macOS, a ref without an element address that was taken with `--surface focused` from a window now resolves only while the window it was captured from is still the app's focused window. It used to re-root at whichever window had focus, so after focus moved to another document of the same app it could act on a same-named control there; it now returns `STALE_REF` with `details.kind: "focused_window_changed"` and sends nothing. `snapshot --surface focused` fails the same way when the window it reads is not the focused one, as with `--window-id` of an unfocused window or when focus moves during the read. ([#80](https://github.com/gshost1/agent-desktop/pull/80))
* An interaction command whose deadline had already run out when it took a free in-process interaction lease now fails `TIMEOUT` with `details.kind: deadline`, `timeout_ms` and `elapsed_ms`. It used to report `interaction_process_lock_timeout` with `contention_count: 0`, which sent agents looking for another process holding a lock nobody held. A wait that found the lease held still reports `interaction_process_lock_timeout` with its count. ([#102](https://github.com/gshost1/agent-desktop/pull/102))
* `wait --event surface-appeared` fires for a surface that was open when the wait began, closed, and reopened under the same id. It used to diff against the frozen start baseline and time out. ([#94](https://github.com/gshost1/agent-desktop/pull/94))
* On Windows, a `launch` whose program exits before its process shows a window, such as a stub that starts another program and exits (`calc` on Windows 11), now fails on the next poll with `APP_UNRESPONSIVE`, the code the shared launch reference gives a process that exits before it shows a window, with `details.kind: exited` (`crashed` for an NTSTATUS crash code), the status and the pid, `delivered_unverified`, instead of polling the dead pid for the whole `--timeout` and answering `ok` with it. An attach to a running instance that shows no window returns after one read instead of waiting out `--timeout`. ([#87](https://github.com/gshost1/agent-desktop/pull/87))
* On Linux, `launch` in a session with no display server that answers now fails `not_delivered` before anything is spawned: `PLATFORM_NOT_SUPPORTED` with `details.display_unavailable`, or `TIMEOUT` for an X server that does not answer. It used to start the program, for example a headless Chrome that stayed running, and then fail with `delivered_unverified` and no pid. A failure after the spawn now names the spawned process in `details.spawned_pid`. ([#90](https://github.com/gshost1/agent-desktop/pull/90))
* On Linux, an AT-SPI service that exits without answering, as the bus launcher and the registry do when started with an unreachable `DISPLAY`, is no longer reported as `INTERNAL` "The accessibility bus returned an unexpected error". When `DISPLAY` or `WAYLAND_DISPLAY` names a display that does not answer, commands answer the display refusal (`PLATFORM_NOT_SUPPORTED`, `details.display_unavailable`, the bus error in `details.bus_error`); commands that check permissions first, such as `launch`, answer `PERM_DENIED` with the display named in the suggestion and spawn nothing. Otherwise the answer is a retryable `PLATFORM_NOT_SUPPORTED` with `details.kind: service_exited`. A NoReply that only means the call timed out, and one from an application, keep their own answers. ([#124](https://github.com/gshost1/agent-desktop/pull/124))
* `find --first` gives a ref to a match without an element token only once it has read the list and table rows and proved the match unique. Before, it published a ref that every action refused with `STALE_REF` `capture_incomplete`, and a larger `--timeout-ms` could not help. When the budget runs out first, the match comes back with its bounds and no ref. ([#118](https://github.com/gshost1/agent-desktop/pull/118))

### Performance

* On macOS, `press --app` and `press --window-id` set up keyboard synthesis while the target app's accessibility connection opens, instead of after it. The first key event a process builds on its main thread loads the keyboard layout (9-10 ms); `press` now builds a throwaway event right after it confirms the app, while the app's first accessibility reply is still on its way. The event it posts is unchanged and still carries its characters. `press` medians fell 3-28 ms on five apps. ([#109](https://github.com/gshost1/agent-desktop/pull/109))
* On macOS, the report of surfaces left open after a `click` reads the focused element's subtree once. The search for an open context menu walked the focused element and then every window, which holds the focused element again. Reported surfaces are unchanged. A Weather `click` makes 203 accessibility calls instead of 286 and its median fell 21 ms, with its slowest run 202 ms instead of 455; Script Editor fell 15 ms; Calculator and the SwiftUI fixture are unchanged. ([#112](https://github.com/gshost1/agent-desktop/pull/112))
* On macOS, look an app up by name (`--app`) by reading full records only for the running apps with that name. AppKit fetched every app's process id, bundle id, executable and launch date from LaunchServices on each read, and a lookup reads the inventory twice to confirm it, and again before a key press or a close. When no app has the name, the same read returns every app, so bundle-id and executable aliases match as before. `press --app` medians fell 10-19 ms and `close-app` 14-23 ms on five apps; `launch` is unchanged. ([#108](https://github.com/gshost1/agent-desktop/pull/108))
* On macOS, a command that acts on or reads a ref (`click`, `get`, `press --ref`, `wait --element` and the other ref commands) opens the accessibility connection to the process its ref was captured from before the permission checks, using the process id saved in the ref map, so the app's first reply overlaps them. `press --ref` also opens the window-server connection its key events need on a background thread. `get` finishes 12-15 ms sooner, `click` up to 19 ms and `press --ref` 30-35 ms on a warm app. ([#113](https://github.com/gshost1/agent-desktop/pull/113))
* On macOS, `close-app` wakes when the kernel reports the app's exit instead of at the end of a fixed 25 ms pause between identity checks. The command returned up to 19 ms after the exit and now returns within 3 ms of it. ([#116](https://github.com/gshost1/agent-desktop/pull/116))
* On macOS, sync private state files (ref maps, the latest-snapshot pointer, session manifests) with `fsync` instead of `F_FULLFSYNC`, which flushes the drive's whole cache and cost about 6 ms per file. Each file is still written, synced and renamed into place, so a killed process or a crashed system sees the old or the new file whole. A power loss before the drive empties its cache can lose the newest snapshot or leave its ref map unreadable; take a new snapshot. `snapshot`, `find` and `wait --text` medians fell 6-26 ms on five apps; commands that store nothing are unchanged. ([#107](https://github.com/gshost1/agent-desktop/pull/107))
* On macOS, read an element's children with one call instead of three where the list fits on one page. Each child read counted the children, read them and counted again, and the count is where the app builds the child list. A single page now asks for one child more than the count, which proves the count without a second one, and container roles (groups, cells, rows, scroll areas and the like) read the children first and count only when that page comes back full or empty. Results are unchanged. `snapshot`, `find` and `wait --text` medians fell 40-48 ms on System Settings, 24-27 ms on Weather and 6-7 ms on the SwiftUI fixture; Calculator and Script Editor are unchanged. ([#110](https://github.com/gshost1/agent-desktop/pull/110))
* On macOS, `launch` reads a launching app's windows at most 25 ms apart instead of 50 ms, starting 10 ms after the first read. It returns at the first read that shows the window placed, so the wait after the window appears is about half as long: the last pause before that read fell from 53-55 ms to 29-30 ms on cold launches. ([#117](https://github.com/gshost1/agent-desktop/pull/117))
* `find --text`, `--has` and `--has-not` read the window once and publish their matches from that read, the way `snapshot` does, instead of walking the window and then walking each match's subtree again. Matches and their order are unchanged. Medians fell 696 ms on System Settings (992 to 296), 432 ms on Weather (710 to 278) and 31 ms on Calculator (148 to 117); `find` by role and name is unchanged. On the SwiftUI fixture and Script Editor `find --text` failed `TIMEOUT` `locator_incomplete` in all 7 runs on both builds, so their 350 ms and 12 ms drops are in the time to that failure, not to an answer. ([#105](https://github.com/gshost1/agent-desktop/pull/105))
* On macOS, the open-menu check after a ref action asks a busy element for its role and children in one batched read once, then reads it one attribute at a time, instead of retrying the batch up to three times first. The attribute reads keep their own retries, so a busy element fails or is skipped as before. ([#115](https://github.com/gshost1/agent-desktop/pull/115))
* On macOS, a command that reads the window list (`snapshot`, `find`, `wait` for an element, window or text, `press --app`, `launch`, `screenshot` of an app or window, and the window commands) opens its window-server connection on a thread before its permission check instead of after it, and command preflight no longer probes Automation in the CLI. Both were meant to ship earlier but never ran there: the browser-page wrapper around the platform adapter passed the full permission report through and dropped the preflight one. Medians on five apps: `snapshot -i` 7-19 ms faster, `find` 14-17 ms, `wait --text` 15-22 ms, `press --app` 17-29 ms; `click`, `get` and `close-app` take no connection they would not use and are unchanged. ([#104](https://github.com/gshost1/agent-desktop/pull/104))
* On macOS, read each element's role and children in one accessibility round trip when a ref action checks whether a menu opened, instead of a role read, a child count, a child page and a recount. Visibility is still read only for a menu, and an element whose batched answer is unclear is read the old way. `click` medians fell 40-51 ms on System Settings, 56-89 ms on Weather and 8 ms on the SwiftUI fixture, and stayed level on Calculator and Script Editor. ([#101](https://github.com/gshost1/agent-desktop/pull/101))
* On macOS, take the capture that confirms an app or window inventory right after the first one instead of 5 ms later. The pause now follows only a capture that changed or failed. `list-windows` fell 3-7 ms and `press` 3-16 ms (medians on five apps). ([#98](https://github.com/gshost1/agent-desktop/pull/98))
* On macOS, a command scoped to an app by name (`list-windows --app`, `snapshot --app`, `find --app`, `focus-window --app` and others that list the app's windows) reads full AppKit records only for the apps with that name when it joins windows to their owners, instead of every running app's twice. It falls back to every record when no app has that display name, so a bundle-id or executable alias still matches. `list-windows --app` finishes 6-11 ms sooner and `snapshot`, `find` and `focus-window` 4-11 ms. ([#119](https://github.com/gshost1/agent-desktop/pull/119))
* On macOS, check a process's code-signing identity before copying out its environment when looking for command hosts left by earlier versions. Every command scanned the environment of every process the user runs (342 on the test Mac) first; the identity rules out nearly all of them in one cheap call. `--version` fell from 7.72 to 6.19 ms and `skills list` from 8.02 to 6.37 ms (medians of 61 and 31 interleaved pairs). A process is still stopped only after the same marker, start-time and identity checks. ([#96](https://github.com/gshost1/agent-desktop/pull/96))
* On macOS, open the window-server connection, the accessibility connection to the target app and the two permission checks on background threads instead of one after another, for commands that read the desktop. Every command is a fresh process and paid 25-40 ms for each connection before it read anything. Medians on a warm app (Calculator, the SwiftUI fixture, Script Editor, System Settings, Weather) fell 25-38 ms for `list-windows`, 26-40 ms for `snapshot -i`, 28-42 ms for `find`, 19-37 ms for `press` and 9-13 ms for `get`, `set-value` and `click`. ([#95](https://github.com/gshost1/agent-desktop/pull/95))
* On macOS, count the children of an element that has none once instead of twice. The second count only proves a list did not change while it was read, and a childless element has no list to read. Snapshot, `find` and the settled-surface walk after a click make one accessibility round trip fewer per leaf: medians fell 10-17 ms on the SwiftUI fixture and Weather and 2-4 ms on smaller trees, with identical trees. ([#97](https://github.com/gshost1/agent-desktop/pull/97))
* On macOS, wait the 2 s accessibility settle only for a Chromium process, recognized by a bundled framework that ships the V8 startup snapshot or is named `Electron Framework` or `Chromium Embedded Framework`. The first snapshot of a freshly launched native app no longer pays it, and `launch` now reports `renderer: "chromium"` for browsers and renamed Electron builds too. ([#5](https://github.com/gshost1/agent-desktop/pull/5))
* `--lines` leaves out `desc=` when the description is the same as the name. weather.gov's home page, with 253 image-map links whose title repeats their text, prints 30% fewer bytes with `-i --lines` (39,237 → 27,569 bytes; 19,310 → 12,801 tokens, −34%), with the same 326 refs. The JSON tree, `find` and the refmap keep the description. ([#84](https://github.com/gshost1/agent-desktop/pull/84))
* `skills get web` now returns a short quickstart, 1,818 tokens instead of 4,369, which every web agent loads first and carries on every later turn; the long guide is `skills get web reference`. The quickstart tells agents to read snapshots with `--lines` (about a third fewer tokens than the JSON tree), to write batch refs literally, and to read a batch back by `failed_entries` and `not_started_entries`. ([#92](https://github.com/gshost1/agent-desktop/pull/92))
* On Linux, the app-name lookup behind `--app`, `--window-id` and `list-windows` resolves only the desktop entries whose command words can run the process, instead of every installed entry through PATH on every call, and computes names and desktop ids in one pass. In a private session with gnome-calculator, one call fell from 1,495 to 27 `readlink` and from 2,848 to 925 `statx` calls, and `list-windows` from 15.9 to 14.1 ms. Results are unchanged. ([#120](https://github.com/gshost1/agent-desktop/pull/120))

### Documentation

* Add a synopsis of `press`, `find`, `type`, `select` and `set-value` to the interaction reference, loaded on demand, with tests that keep each form in step with the command's flags. Taken from [#48](https://github.com/gshost1/agent-desktop/pull/48) without its additions to the core skill, which in a paired run made the three bench tasks use 14% more input tokens. ([#82](https://github.com/gshost1/agent-desktop/pull/82))
* Tell agents to take a path the task names, and the answer, in one call: `find` each step by name, `click` it with `-w` naming the next step, and end with a `grep`-filtered read, never `head`; `jq -e` stops the chain at the first miss. On the three macOS bench tasks (Opus 5.5, 8 runs per task per build, paired against main cce8fa1c) System Settings fell from 4 to 2 turns, 51.1K to 25.8K input tokens and 19.2 s to 12.4 s (medians); the summed medians went from 48.6 s to 43.2 s and 119.0K to 95.4K tokens, 24 of 24 passing on both. ([#85](https://github.com/gshost1/agent-desktop/pull/85))

### Testing

* The SwiftUI fixture's double and triple targets now register a headed `double-click` and `triple-click`, which the macOS E2E steps "headed double-click delivers the physical gesture" and "headed triple-click delivers a three-tap gesture" check. The targets were bordered buttons, whose AppKit mouse tracking takes the mouse-up before the app's multi-tap gesture sees it, so neither gesture could finish; they now use the plain button style. `double-click`, `triple-click` and `mouse-click --count` were not at fault: the same clicks reach an AppKit view as `clickCount` 1, 2 and 3 and select a word and then a line in a native text field. ([#127](https://github.com/gshost1/agent-desktop/pull/127))
* The Linux no-display launch test stops its fixture by the recorded pid and runs on a private session bus. ([#122](https://github.com/gshost1/agent-desktop/pull/122))

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
