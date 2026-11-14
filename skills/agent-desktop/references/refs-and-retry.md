# Refs, delivery and retry

The full rules behind the short loop in the core skill: how a ref is re-identified, what each ref error means, when an action may be repeated, and what headless and headed mode allow.

## Map, drill and read

- `-i` keeps interactive nodes only, so a row's static-text label is dropped. To tell which row a repeated button belongs to, drill that row's container with `--root` and leave out `-i`. On Windows and Linux that reads the row but does not make its button actionable: a button repeated in other rows is a twin in the window and its ref is refused (see Refs).
- A `--root` drill adds its refs to the same snapshot id. Refs outside the drilled subtree, and the drilled root itself, stay valid; refs that were already inside it are replaced, so an old ref there returns `STALE_REF`.
- `--skeleton` gives a shallow map with `children_count` on cut branches. A small app needs no skeleton; take a full `snapshot -i`. Never skeleton a surface (`--surface menu`, `sheet`, `alert`): a surface is already focused.
- An action that opens a menu, sheet or alert lists it in `data.surfaces`. Target that overlay next. The report gets at most one second, so in an app with a large tree it can be missing or carry `unclassified`; when you expect a dialog, snapshot `--surface sheet` or `--surface alert`.
- Each line of `data.lines` is one node, indented by depth: its ref first, then role and quoted name. Take the ref at the start of the line that names the control, never from a neighbouring line. Without `--lines`, check each ref against its node's `name`. A wrong ref costs a corrected rerun, and a rerun is safe only when `disposition.retry` is `safe`.

## Refs

- Output refs are qualified: `@<snapshot_id>:e<N>`, for example `@s8f3k2p9:e1`. Pass them to later commands as they are.
- Refs are valid for one snapshot. After a UI change, re-drill the affected region or take a new snapshot before you use refs inside it.
- Only addressable elements get refs. Static text and inert containers do not.
- A command re-identifies the element before it acts. On macOS the ref carries the element's own address, so identical twins each keep their ref. On Windows and Linux a ref resolves only when it was the only element in the window its snapshot captured with its role, native id, name and description, counted even below a skeleton or depth cut, and exactly one live element still matches. Identical twins there have no ref path: only `--headed mouse-click --xy X,Y` can choose one, it takes the foreground, and a Linux Wayland session refuses it.
- `STALE_REF`: the element is gone or no longer matches. `details.kind` is `saved_ref_absent`, or `retained_object_stale` on macOS. Take a fresh snapshot and use the new ref. `identity_unproven` means no capture proved the ref: with `details.reason: capture_incomplete` the snapshot could not read the whole window, so retake it with a larger `--timeout-ms` (the same timeout falls short again); with `capture_unknown` the element's identity could not be read, so use a different selector or `--headed mouse-click --xy`.
- `AMBIGUOUS_TARGET`: an identical element appeared (`saved_ref_ambiguous`), or the snapshot found an identical twin (`identity_not_unique`). Repeating the ref or re-snapshotting identical twins gives the same refusal; act on a control that is unique in the window. `identity_not_unique` on a ref from a snapshot taken before version 2.7 may not be a twin: take a new snapshot first.
- `TIMEOUT` with `details.kind: resolution_identity_unknown`: a live read did not answer, so identity is unproven. Nothing was sent; retry.
- `SNAPSHOT_NOT_FOUND`: the snapshot id expired. Snapshot again.

## Delivery and retry

Action results carry `data.disposition` and errors carry `error.disposition`, each with `delivery` and `retry`. Observation commands such as `snapshot` have none on success; their errors, and every `INVALID_ARGS` refusal, carry `not_delivered` and `retry: safe`.

- `delivered_verified` means the action read its effect back. `delivered_unverified` means the dispatch completed without that read. Neither needs a separate snapshot to prove delivery; read the UI again when the next step needs new refs or text, or when the task's outcome depends on an effect the result could not verify.
- Retry or substitute an action only when `disposition.retry` is `safe`. Only `not_delivered` is safe.
- Any other delivery value means the action may have landed. A repeat can click, type or submit twice, and `toggle` can undo itself.
- On a result that is not safe, read the live state (`get`, `is`, or a fresh snapshot) and decide from that. `details.post_state` helps when present.
- Branch on `error.code` and `error.disposition`. Message and suggestion text can change.
- `details.retryable: false` means waiting cannot change the outcome. Try a different approach.

## Headless and headed

Ref actions are headless by default. They use semantic accessibility calls and do not take focus, move the cursor, synthesize keys or use the clipboard. If no semantic path exists, the action fails closed. Use the global `--headed` flag only when the user explicitly permits physical input, such as double-click, hover, drag, or focus. Never switch to headed mode to recover from a headless failure. Raw coordinates (`--xy`) never imply focus. Which actions need `--headed` differs by OS: read the platform skill.
