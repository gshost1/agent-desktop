# Core Rules

The detail behind the core quickstart's short rules: shell syntax, which skill to load when, scoping a snapshot, ref identity, delivery and retry, and headless input. The quickstart's lines still apply; this reference adds what they leave out. Load it when a ref is refused, a retry is not `safe`, or you work in PowerShell.

## Shell syntax

Examples use a POSIX shell. PowerShell needs the ref quoted because a bare `@token` is its splatting operator, which drops the ref from the arguments. Use `$env:NAME = 'value'` instead of `export`, and `| ConvertFrom-Json` instead of `$(...)` and `jq`. The Windows skill has the PowerShell forms.

## What to load when

- Load the core quickstart (`agent-desktop skills get desktop`) before the first action. It is the only skill an agent needs up front, and `status` is not a required first step.
- Load `agent-desktop skills get platform` on `PERM_DENIED`, an empty tree or another OS-specific error, and before OS-specific work such as granting permissions, shell surfaces or notifications. It serves the skill for the OS the binary runs on (macOS, Windows or Linux).
- Run `agent-desktop status` when you need the platform, the permission report or `supported_surfaces`.
- For a flag the quickstart does not show, run `agent-desktop <command> --help`.

## Scope a snapshot

Use `find` when you know the role or exact name. Otherwise map the app first and drill into one region.

```bash
agent-desktop snapshot --skeleton --app "Notes" -i --compact   # shallow map, children_count on cut branches
agent-desktop snapshot --root @s8f3k2p9:e3 -i --compact        # drill into one region
agent-desktop click @s8f3k2p9:e12                              # act on a qualified ref
agent-desktop snapshot --root @s8f3k2p9:e3 -i --compact        # verify: re-read the same region
```

- A small app needs no skeleton. Take a full `snapshot -i`.
- `-i` keeps interactive nodes only, so a row's static-text label is dropped. To tell which row a repeated button belongs to, drill that row's container with `--root` and leave out `-i`. On Windows and Linux that reads the row but does not make its button actionable: a button repeated in other rows is a twin in the window and its ref is refused (see Refs).
- A `--root` drill adds its refs to the same snapshot id. Refs outside the drilled subtree, and the drilled root itself, stay valid. Refs that were already inside it get new ids, so an old one there returns `STALE_REF`.
- A surface (`--surface menu`, `sheet`, `alert`) is already a small, focused tree, so `--skeleton` only cuts it short.
- `wait` and its predicates are in `commands-system`.

## Refs

- A ref has the form `@<snapshot_id>:e<N>`, for example `@s8f3k2p9:e1`. A bare `@e1` is refused with `INVALID_ARGS`.
- After a UI change, re-drill the affected region with `--root` rather than taking a full snapshot.
- Only addressable elements get refs. Static text and inert containers do not.
- A command re-identifies the element before it acts. On macOS the ref carries the element's own address, so identical twins each keep their ref. On Windows and Linux a ref resolves only when it was the only element in the window its snapshot captured with its role, native id, name and description, counted even below a skeleton or depth cut, and exactly one live element still matches. Identical twins there have no ref path: only `--headed mouse-click --xy X,Y` can choose one, it takes the foreground, and a Linux Wayland session refuses it.
- `STALE_REF`: the element is gone or no longer matches. `details.kind` is `saved_ref_absent`, or `retained_object_stale` on macOS. Take a fresh snapshot and use the new ref.
- `AMBIGUOUS_TARGET`: an identical element appeared (`saved_ref_ambiguous`), or the ref was not proven unique at capture (`identity_not_unique`). Repeating the ref or re-snapshotting identical twins gives the same refusal; act on a control that is unique in the window. `identity_not_unique` also follows a snapshot that could not read the whole window in time: retake it with a larger `--timeout-ms`.
- `TIMEOUT` with `details.kind: resolution_identity_unknown`: a live read did not answer, so identity is unproven. Nothing was sent; retry.
- `SNAPSHOT_NOT_FOUND`: the snapshot id expired. Snapshot again.

## Delivery and retry

Action results carry `data.disposition` and errors carry `error.disposition`, each with `delivery` and `retry`. Observation commands such as `snapshot` have none on success; their errors, and every `INVALID_ARGS` refusal, carry `not_delivered` and `retry: safe`.

- The `safe` rule covers substitutes too: don't swap in a different action for one that may have landed. Only `not_delivered` is safe.
- A repeat after any other delivery value can click, type or submit twice, and `toggle` can undo itself.
- `delivered_verified` means the adapter read back the postcondition. `delivered_unverified` means the dispatch completed with no readback, so `ok: true` does not prove the effect.
- To decide after an unverified or unsafe result, read the live state of the control: `get`, `is`, `find`, or a `--root` drill of its region. `details.post_state` helps when present.
- Branch on `error.code` and `error.disposition`. Message and suggestion text can change.
- `details.retryable: false` means waiting cannot change the outcome. Try a different approach.

## Headless and headed

Headless ref actions use semantic accessibility calls only. They also never synthesize keys. If no semantic path exists, the action fails closed instead of falling back to physical input. `--headed` covers physical input such as double-click, hover, drag and taking focus. Raw coordinates (`--xy`) never imply focus. Which actions need `--headed` differs by OS: read the platform skill.

## Which reference to open

| Reference | Open it when |
|-----------|--------------|
| `platform` (`agent-desktop skills get platform`) | `PERM_DENIED`, an empty tree, an OS-specific error, or before OS-specific work |
| `commands-observation` | You need `snapshot`, `find`, `get`, `is`, `screenshot`, `list-surfaces` semantics, partial trees, or the `--root` rules |
| `commands-interaction` | You choose between click, type, set-value, select, toggle, scroll, press and mouse commands, or you debug an action error |
| `commands-system` | You launch or close apps, manage windows, use the clipboard, `wait`, `batch`, sessions, multi-agent cursors, tracing, or load skills |
| `workflows` | You want worked patterns (forms, menus, dialogs, waiting for a new window, scroll-find, drag) and the anti-patterns list |
