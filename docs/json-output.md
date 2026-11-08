# JSON output contract

Every command returns structured JSON:

```json
{
  "version": "2.6",
  "ok": true,
  "command": "click",
  "data": {
    "action": "click",
    "disposition": { "delivery": "delivered_unverified", "retry": "unsafe" }
  }
}
```

Errors include machine-readable codes and recovery hints:

```json
{
  "version": "2.6",
  "ok": false,
  "command": "click",
  "error": {
    "code": "STALE_REF",
    "message": "@s8f3k2p9:e7 not found in current RefMap",
    "suggestion": "Re-run a snapshot to obtain fresh refs, then retry with the new ref (CLI: snapshot [--skeleton]; FFI: ad_snapshot then pass the printed qualified ref to ad_execute_by_ref)",
    "recovery": {
      "strategy": "refresh_snapshot_then_retry_original",
      "retryable": true,
      "requires_fresh_snapshot": true
    },
    "disposition": {
      "delivery": "not_delivered",
      "retry": "safe"
    }
  }
}
```

`error.code`, `error.message` and `error.disposition` are always present. `suggestion`, `recovery`, `platform_detail` and `details` are omitted when empty. `recovery` appears only when `disposition.retry` is `safe`, and its `retry_after_ms` is set only for `TIMEOUT` and `APP_UNRESPONSIVE`.

Version 2.2 replaces the 2.1 `TIMEOUT` error for a budget-exhausted snapshot
with a successful, explicitly partial response, and adds `data.complete`.
Snapshot data always carries `complete`. A snapshot that runs out of budget now
returns `ok: true` with `"complete": false`, the tree it did observe,
`"truncated": true`, and `nodes_observed`, instead of a `TIMEOUT` error and no
tree. Every node whose descendants were cut short carries
`"subtree_truncated": true`, so a reader can walk from the root to each
boundary. Consumers must read `data.complete` to decide whether a tree is whole;
a `TIMEOUT` error no longer signals an oversized tree. A `--root` drill-down is
unaffected: it replaces refs inside an existing snapshot, so an incomplete
observation still returns `TIMEOUT` rather than a partial tree.

Version 2.1 replaces the 2.0 `error.retry_command` string with the structured
`error.recovery` object and adds `error.disposition`. Consumers must select a
recovery strategy from `recovery.strategy` only when `disposition.retry` is
`safe`; command strings from older envelopes must not be executed blindly.
The removed `retry_command` field has no compatibility alias.

## Disposition values

Every error carries `disposition`, and commands that act on a target may also
carry it on their result. `delivery` is one of `unknown`, `not_delivered`,
`delivery_uncertain`, `delivered_unverified` or `delivered_verified`. `retry`
is one of `unknown`, `safe` or `unsafe`, and it follows from `delivery`:
`not_delivered` is `safe`, and every other non-`unknown` value is `unsafe`.

`unknown` means the command established nothing about delivery. A failure of a
command that sends input to an application and carries no delivery evidence,
such as a `TIMEOUT` on an operation that may already have landed, keeps it. A
read-only command sends nothing, and neither does a command that only writes a
local file or local state (`screenshot <PATH>`, `clipboard-get --out`,
`trace export` and `session gc`), so their errors carry `not_delivered`.
`session start` and `session end` change session state and drive the cursor
overlay, so they are treated like commands that send input. Invalid arguments never reach the application, so
their errors, and a lookup failure such as `APP_NOT_FOUND`, carry
`not_delivered` too. The exception is an `INVALID_ARGS` error with no
disposition of its own that carries `details.preparatory_focus` or
`details.preparatory_scroll`: a focus or scroll ran before the refusal, so it
keeps `unknown`. Treat `unknown` as not safe to retry automatically: read
`error.code` and the message, and choose a recovery strategy only when
`disposition.retry` is `safe`.

A `batch` entry whose result does not fit the batch output limit is replaced by
an `INVALID_ARGS` error with `details.result_omitted: true`. It keeps the
entry's own disposition: the result's `data.disposition` or
`error.disposition`, `not_delivered` for a command that sends no input, and
`delivered_unverified` for a successful command that sends input and reports
none.

Under `--trace-strict`, a `batch` whose closing trace record cannot be written
fails with the trace error and `details.kind: "batch_trace_end_failed"`. Every
entry has already run, so `details.batch` holds the batch body with each
per-entry result, and `details.cause_details` keeps the trace error's own
details. A body that would push the response past the batch output limit is
left out: `details.result_omitted` is `true` and only `total_entries`,
`completed_entries`, `not_started_entries` and `stopped` are kept. The
disposition is `not_delivered` when no entry can have delivered (each entry's
disposition is `not_delivered`, or it is a successful command that sends no
input), and `delivery_uncertain` otherwise. Any other command whose closing
trace record fails after it succeeded reports `delivered_unverified` unless it
is read-only.

A ref action that fails after it starts waiting on the interaction lease and
before it sends the action carries `not_delivered`. That covers a `TIMEOUT`
waiting for the lease, a deadline spent while taking it, a failed headed focus
step, a failed resolve or actionability check under the lease, and a failed
trace write at dispatch start. The focus step of a headed pointer command, and
the lease, a deadline spent while taking it, target revalidation and focus step
of `press`, are `not_delivered` the same way. A key refusal that `press`
raises after its headed focus carries `details.preparatory_focus` too. Once a focus call has run, a
failure raised later in the ref action's dispatch (the focus itself, the resolve
and actionability check under the lease, the action and its verification)
carries `details.preparatory_focus: "may_have_moved_focus"`, because that call
may have raised a window; its disposition still says whether the action was
sent. A failure before the focus call, such as a window that no longer matches,
carries no such detail, and neither does a failure in a step after the action,
such as a `--wait-for` condition or writing the result. A ref action makes at
most one focus attempt: once a focus call has run it does not go back to
waiting, even when the error is retryable.

On macOS, `press --app` and `press --window-id` with `return`, `enter` or
`escape` and no modifiers check the effect when the focused window is a sheet
or dialog, or carries a sheet. A real key goes first, before any semantic
action, because `AXConfirm` on a text field ends editing and moves focus off
it, after which the sheet ignores Return. The press is verified only by a
postcondition the window server shows, held on two consecutive reads within
750 ms: `surface_window_closed`, the sheet's or dialog's own window has left
the screen. `text_edited` also counts, but only for a multi-line text area.
Accessibility listings, window size and title changes never count. When the
real key cannot be sent or the surface's window stays on screen, the focused
control's `AXConfirm` or `AXCancel` runs and is judged the same way. A
verified press is `delivered_verified` with `details.modal_key: { "surface":
"sheet" | "dialog", "effect": "observed", "signal": "<signal>",
"observed_after": "<step label>" }`. When the surface's own window cannot be
told apart from the window it is attached to, only the real key is sent and
the result is `delivered_unverified` with `modal_key.effect: "unobservable"`.
A surface still on screen after both routes is an `ACTION_FAILED` error with
`delivered_unverified` delivery, `retry: unsafe` and
`details.modal_key.effect: "none_observed"`, plus `routes` (the step labels
tried) and `default_button` (for Return) or `cancel_button` (for Escape), which
says whether the surface names such a button. Read the surface again and use
its controls by ref instead of repeating the key. Presses with modifiers, other
keys, and presses with no sheet or dialog in the focused window are unchanged.

## Version 2.4 migration

Stateful ref actions (`set-value`, `type`, `clear`, `check`, `uncheck`, `toggle`, `expand`, and `collapse`) now reject observed postcondition contradictions after one delivery attempt. Readback failures preserve the adapter's error code and carry `details.kind: "post_action_verification"`. Missing evidence alone remains a successful `delivered_unverified` result with `details.verification_scope: "unavailable"`; neither that result nor verified element state proves application-level persistence.

macOS mutation errors that do not prove non-delivery stop fallback chains with an unsafe retry disposition. This affects activation, selection, disclosure, scrolling, and window operations; an unchanged or unreadable state is not proof that a mutation was never delivered. Value-writing chains also stop after unverified delivery instead of applying a second mechanism. Failures after preparatory scrolling carry `details.preparatory_scroll`, which records that the view may have changed. Their retry disposition follows the failure: a refusal that sent nothing is `not_delivered` and safe to retry, and any other failure keeps its own disposition.

Inspect `disposition.retry` before retrying or choosing an alternative action, regardless of `error.code`. An error may include `post_state` and `postcondition_satisfied` without erasing the original execution failure. The same verification rules apply to FFI ref actions; the C ABI is unchanged. The Rust `ActionResult::from_execution` constructor now takes only action and steps and returns an infallible result; live verification belongs to `execute_verified_action`.

## Version 2.5 migration

Version 2.5 is a breaking payload change for the commands that dispatch pointer, drag, clipboard, app-close and window operations: `mouse-click`, `mouse-move`, `mouse-wheel`, `hover`, `drag`, `clipboard-set`, `clipboard-clear`, `close-app`, `move-window`, `resize-window`, `minimize`, `maximize` and `restore`.

The unconditional success flags are removed with no compatibility alias: `clicked`, `moved`, `scrolled`, `hovered`, `dragged`, `resized`, `minimized`, `maximized`, `restored`, `closed`, `cleared`, the `requested` field of `close-app`, and the inner `ok` of `clipboard-set`. A flag that every successful call carried proved nothing about the operation.

A verified window report names the exact readback in `details.verification_scope` (`window_size`, `window_position`, `window_minimized_state` or `window_maximized_state`). Geometry scopes also state `details.tolerance` and `details.units`: 2 points on macOS, 8 px on Windows, 2 pixels on Linux. A window failure after the write keeps its code, details and disposition and gains `details.target` and the completed `details.steps`.

Each of these commands now returns the adapter's own report inside `data`:

```json
{
  "action": "drag",
  "disposition": { "delivery": "delivered_unverified", "retry": "unsafe" },
  "steps": [ { "label": "pointer_drag", "outcome": "succeeded", "mechanism": "physical_synthetic" } ],
  "target": { "kind": "drag", "from": { "x": 10, "y": 20 }, "to": { "x": 80, "y": 20 } },
  "from": { "x": 10, "y": 20 },
  "to": { "x": 80, "y": 20 }
}
```

- `disposition.delivery` is `delivered_verified` only after the adapter read back the postcondition it names, `delivered_unverified` after a completed dispatch without that proof, and `not_delivered` only when the adapter observed that the operation was already satisfied (for example `close-app` on an application that had already exited). `ok: true` means the command returned a delivered or already-satisfied result, not that the application effect was verified.
- `target.kind` is `window` (`id`, `title`, `process`), `app` (`name`, `process`), `pointer` (`point`), `drag` (`from`, `to`) or `clipboard`. The adapter supplies the identity it validated and used. A pointer target is the dispatch coordinate and does not prove which application received the input.
- Request context (`x`, `y`, `count`, `dx`, `dy`, `width`, `height`, drag endpoints, `drop_delay_ms`, `type`, `app`, `method`, and a confirmed `focused`) keeps its earlier meaning. These fields echo the request and are not observed post-state.
- Errors keep `error.disposition`. A failed `--wait-for` after a delivered operation keeps the operation's own delivery (verified, unverified, or not delivered) in `error.disposition` and carries the report under `details.after_action`.
- The Rust `ActionResult` and `OperationReport` readers reject a payload without an explicit `disposition`; a missing disposition no longer reads back as delivered-unverified.
- The C ABI is unchanged. Status-only functions such as `ad_close_app` and `ad_drag` still cannot expose the disposition, steps or target of a successful call.

## Version 2.6 migration

Version 2.6 changes which refs resolve on Windows and Linux, and on macOS for an element that yields no address. Such a ref resolves only when one complete read of its window proved it unique when it was captured and exactly one live element matches its role and stable fields now. The only new payload fields are `data.identity_unproven` and, on a ref action whose budget ran out after a refusal, `error.details.refusal`.

- A ref to one of two identical controls, which earlier resolved by position or geometry, now fails `AMBIGUOUS_TARGET` with `details.kind: "identity_not_unique"` before anything is read. So does a ref captured from a window that could not be read whole within the snapshot timeout.
- A refmap written before 2.6 records no capture proof, so all of its refs on these platforms fail the same way. Take a new snapshot.
- An identical twin that appears after capture fails `AMBIGUOUS_TARGET` with `details.kind: "saved_ref_ambiguous"`.
- All of these carry `details.retryable: false` and a `not_delivered` disposition. `disposition.retry` stays `safe` because nothing was sent, but repeating the same ref returns the same refusal.
- When a ref action's full resolution under the lease is refused in a way that may clear, the action waits again within its `--timeout-ms`. If that budget then runs out, the error carries the refusal in `details.refusal`: its `code`, `message`, and `details` when the refusal had any.
- A surface kind the OS does not have now returns `INVALID_ARGS` instead of `PLATFORM_NOT_SUPPORTED`, with the disposition `not_delivered` and `retry: "safe"`. FFI callers see the matching `AdResult` code change. An adapter with no surfaces at all still returns `PLATFORM_NOT_SUPPORTED`.
- On macOS, `open-system-surface --surface dock` and `--surface menu-bar-extras` now succeed headless. Their `data` has no `window`; check for the field before reading it. Windows and Linux still return `window` and need `--headed`.
- On macOS, a snapshot of a background app that lists no window (a menu-bar app, the Dock, Control Center), and `--surface dock` or `--surface menu-bar-extras`, now succeed and read the app through its process. Earlier they failed `WINDOW_NOT_FOUND`. `data.window` is omitted from such a snapshot and `data.app` names the process; check for the field before reading `data.window.id`.

A snapshot that needed one complete read of its window to prove its refs, and could not get it, says so with `"identity_unproven": true` in `data`. `complete` still describes the presented tree. On Windows and Linux, and on macOS for an element with no address, every ref of that snapshot fails `AMBIGUOUS_TARGET` with `details.kind: "identity_not_unique"`; retake the snapshot with a larger `--timeout-ms`. The field is omitted otherwise.

On macOS, application-owned `menu` and `menubar` snapshots omit `data.window`. `data.app` identifies the owner, including for a `--root` drill into a ref from one of these surfaces. Window-owned snapshots retain `data.window`. When `--window-id` is supplied for an application-owned surface, it selects the owning app while the observed surface remains application-owned.

A complete macOS menu lookup with no surface returns `ELEMENT_NOT_FOUND` with `details.kind: "surface_absent"`, `details.surface: "menu"` or `"menubar"`, and `details.complete: true`. That is completed absence evidence. An absent `menubar` can mean the application exposes no menu bar. A `menu` lookup requires an open menu: snapshot `menubar` when it exists, click its owning item without `--headed`, then snapshot `--surface menu`. Missing `complete`, `complete: false`, a timeout, or an incomplete AX read leaves the result unknown, not absent. If a prior action has `delivery_uncertain` and `retry: unsafe`, inspect the current UI before any action; do not repeat the action.

- Focus and preparatory scrolling can change coordinates. Stability is verified against fresh live observations, not by requiring coordinates to remain equal to the original snapshot.
- On macOS, clicking an item in the native menu bar uses `AXPress`. It works headless, and under `--headed` it does not move the pointer. A headless `right-click` can use `AXShowMenu` when the target provides it.

- Focusing an inactive text field can change its selection. A pre-focus range cannot establish the expected insertion into a nonempty field; such a result is explicitly unverified rather than falsely contradicted.
- A settle-loop pacing budget does not replace the caller's deadline for native reads. Tests assert deadline identity rather than arbitrary remaining-time thresholds.

### Post-action observation

When a ref action returns `delivery_uncertain` with a post-action wait selector, `error.details.post_action_observation` records a read-only observation of the original process. Application-owned refs keep their source surface. Window-owned refs observe their original pinned window, including refs captured from `sheet`, `alert`, `popover`, `focused`, or a window-owned `menu` such as a Linux menu. This lets a closed overlay be observed without treating the parent window as absent. The action is never repeated, and its original error and unsafe retry disposition stay unchanged. Even `expected_state_observed` does not prove that the action caused the state. Observation uses `--wait-timeout` separately from the action timeout.

| Field | Meaning |
| --- | --- |
| `kind` | Always `post_action_observation`. |
| `selector, expected` | Requested selector and `present` or `absent`. |
| `surface` | The observed scope: the application-owned source surface, or `window` for a window-owned ref. |
| `process_identity` | Original `pid` and `process_instance`; the instance can be null for `identity_unavailable`. |
| `elapsed_ms` | Time spent observing after the action returned. |
| `source_window_id` | Original window ID, when the ref has one. |
| `selector_matches` | Observed selector result, when available; absence requires a complete read. |
| `process_state` | Last observed `unresponsive`, `exited`, or `crashed` state. `running` is never emitted. |
| `last_observation_error` | Last observation error's `code` and `message`, when available. |
| `outcome` | One of the outcomes below. |

| Outcome | Meaning |
| --- | --- |
| `expected_state_observed` | The expected selector state was observed, or the original window or surface was confirmed absent for an absence wait. |
| `process_exited` | The original process exited; this does not establish the action's effect. |
| `process_crashed` | The platform reported that the original process crashed. |
| `owner_unavailable` | The original surface owner remained unavailable without confirmed process exit. |
| `observation_failed` | A terminal read error stopped observation; inspect `last_observation_error`. |
| `timed_out` | The wait budget expired without observing the expected state, including when transient read errors persisted through the last poll. `last_observation_error` may hold the last error. |
| `identity_unavailable` | The ref has no usable process generation, so observation was skipped. |

## Web channel

The envelope version stays `2.6`. The web channel adds fields and one command; nothing existing changes shape.

`snapshot --surface web --app <browser>` reads the page the browser window shows. When the browser exposes a DevTools endpoint that its own process owns, the page is read over CDP and `data` carries:

| Field | Meaning |
| --- | --- |
| `channel` | `"cdp"`: the tree came from the page over the DevTools endpoint. |
| `url` | The page's URL. |
| `title` | The page's title. |
| `suggestion` | Present only when cross-origin frames could not be read and are left out. |

Without such an endpoint, the same command reads the browser window through the accessibility tree and `data` carries `"channel": "ax"` and a `suggestion` naming the `launch --cdp` command that enables the web channel. Snapshots of every other surface carry no `channel`. Refs from a CDP read carry an element token of the form `cdp:<targetId>:<frameId>:<loaderId>:<backendNodeId>`, are accepted by every ref command, and act through the page without focusing the window or moving the pointer. A web ref whose frame now shows another document (`details.kind: "cdp_document_replaced"`), whose node left the page, or whose role or name changed returns `STALE_REF`; a ref whose tab or frame closed returns `WINDOW_NOT_FOUND` with `details.kind: "cdp_target_gone"`. A click lands on the visible part of the element, on its first line that is not covered when it wraps; an element another element covers at every such point returns `ACTION_FAILED` with `details.kind: "cdp_click_intercepted"` and nothing delivered. Ref action results on web refs carry `details.channel: "cdp"`.

`list-surfaces` adds one entry per open page when an endpoint reaches the browser: `{"id": "<targetId>", "type": "web", "title": "<page title, or its URL>"}`.

`open-url <url> --app <browser> [--new-tab] [--window-id <id>]` loads a URL and waits for the load event. `opened` is `current_tab` when the URL replaced the page the window shows, and `new_tab` for `--new-tab` or for a browser that had no page to replace. `data` is:

```json
{ "url": "https://example.com/", "title": "Example Domain", "channel": "cdp", "target_id": "4C1E...F0", "opened": "current_tab" }
```

Without an endpoint, `open-url` returns `PLATFORM_NOT_SUPPORTED`. A browser that asks its user to approve each DevTools connection returns `PERM_DENIED` with `details.kind: "cdp_approval_required"` when the approval does not arrive in time.

## Find result shape

`find` match objects report the element's actual accessibility `name`. Elements
without an accessible name omit `name` entirely; their content stays in `value`.
Earlier output synthesized a display name, falling back to `value` or
`description`, then a `(unnamed <role>)` placeholder, so consumers that treated
`name` as always present must tolerate its absence and apply their own fallback
to `role` or `value`.

## Ref re-identification details

Every ref is re-identified from its stored entry or refused, and `details.kind` names which. The core resolver refuses a saved ref with one of these four kinds:

| `details.kind` | Code | Meaning |
|------|------|---------|
| `retained_object_stale` | `STALE_REF` | macOS: the element's recorded address is dead, or its process, role or recorded identifier no longer matches. `details.reason` is `dead`, `pid`, `role` or `native_identifier`. No search runs, so the ref cannot land on a look-alike. Take a fresh snapshot. |
| `saved_ref_absent` | `STALE_REF` | No live element matches the stored role and stable fields over complete evidence, so the element is gone. Take a fresh snapshot. |
| `saved_ref_ambiguous` | `AMBIGUOUS_TARGET` | An element identical to the captured one appeared, so two or more live elements match the stored role and stable fields. A fresh snapshot refuses both as twins; act on a control that is unique in the window. |
| `identity_not_unique` | `AMBIGUOUS_TARGET` | The ref was not proven the only element matching its role and stable fields when it was captured, so nothing is searched. Either an identical twin existed, or the window could not be read whole within the snapshot timeout (retake the snapshot with a larger `--timeout-ms`). It is reached on Windows, on Linux, and on macOS when the element yields no address. A twin is refused by every snapshot; only an explicit coordinate click (`--headed mouse-click --xy`) can pick one, it takes the foreground, and it is unavailable on a Linux Wayland session. |

Two outcomes are retryable. `saved_ref_landing_moved` carries `STALE_REF` with `details.complete: false` and `details.retryable: true`: the whole-window count found the element exactly once, but a different element sat at its path when the resolver reached it, as when a list re-sorts during the read. The element was there and only the descent disagreed, so this is not `saved_ref_absent`; the next attempt counts again. A ref action retries it at most twice in a row and answers the third with `saved_ref_absent`. A landing with no count just before it, such as the C find entry point's first-match landing, reports `saved_ref_absent` instead.

The other is `resolution_identity_unknown`, which carries `TIMEOUT`: a live field could not be read, so the identity count stayed unproven. It is retryable. Linux menu and tray refs report the same kind as `STALE_REF`.

After the core resolver chooses the element, the platform adapter lands it, and a landing failure keeps the adapter's own `details.kind` (for example `resolve_no_candidate` on Linux and Windows, or `resolution_complete_absence` on macOS). The platform skills list those kinds. `saved_ref_absent`, `saved_ref_ambiguous` and `identity_not_unique` carry `details.retryable: false`, because waiting cannot change the answer. Every one of these is `not_delivered`.

A refusal without `details.retryable: true` ends every wait at once. With it, what happens next depends on the caller. A ref action (its auto-wait, its retry under the interaction lease, and the pointer actions' waits) treats a known complete absence, a `STALE_REF` with `details.complete: true` and kind `resolve_no_candidate`, `resolution_complete_absence` or `saved_ref_absent`, as final and answers at once, whatever its `retryable` stamp, because the recovery is a new snapshot. That includes a list rebuilt under the lease while the action resolved it. It polls any other retryable resolution refusal within its budget; when the budget runs out on a retryable `STALE_REF` or `AMBIGUOUS_TARGET` read over complete evidence, it reports that refusal with `details.elapsed_ms` instead of the actionability `TIMEOUT`. `wait --element` waits for an element to appear, so it keeps polling every retryable refusal, a complete absence included, until its deadline.

## Error codes

| Code | Meaning |
|------|---------|
| `PERM_DENIED` | Accessibility permission not granted |
| `ELEMENT_NOT_FOUND` | No element matched the ref or query |
| `APP_NOT_FOUND` | No matching application process; the lookup failed before anything was sent, so the disposition is `not_delivered` |
| `STALE_REF` | Ref could not be re-identified in the live UI |
| `AMBIGUOUS_TARGET` | Ref recovery matched multiple plausible targets |
| `SNAPSHOT_NOT_FOUND` | Snapshot ID is missing or expired |
| `POLICY_DENIED` | Physical/headed path blocked by policy |
| `ACTION_FAILED` | Action rejected, or its observed result contradicted the request |
| `ACTION_NOT_SUPPORTED` | The target does not expose the requested action |
| `APP_UNRESPONSIVE` | The matching application stopped responding |
| `PLATFORM_NOT_SUPPORTED` | Adapter method not implemented on this platform |
| `TIMEOUT` | Wait condition expired, or a live identity read stayed incomplete until the deadline |
| `INVALID_ARGS` | Invalid argument values |
| `WINDOW_NOT_FOUND` | The requested window does not exist, including a `screenshot --window-id` that matches no window; the disposition is `not_delivered` |
| `NOTIFICATION_NOT_FOUND` | The notification ID was not found |
| `INTERNAL` | Unexpected error or caught panic |

## Exit codes

`0` success, `1` structured error (JSON on stdout), `2` command-line parse error (the envelope is still printed, with `INVALID_ARGS`).
