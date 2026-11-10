# agent-desktop: web pages, full reference

The quickstart, `agent-desktop skills get web`, covers the loop. This reference holds the details: launching, reading long pages, date and time inputs, batches and every web error. A page in Chrome takes the same snapshot, ref and action loop as a native app. For native apps in the same task (Finder, TextEdit, a Save panel), also read `agent-desktop skills get desktop`.

## Start the browser

```bash
agent-desktop launch "Google Chrome" --cdp --arg=--user-data-dir="$PWD/chrome-profile" --arg=--no-first-run --arg=--no-default-browser-check
```

- `--cdp` needs a fresh browser process. If Chrome is already running, `launch` returns `ACTION_FAILED` with `details.kind` `cdp_requires_fresh_launch`. Closing the user's Chrome loses their tabs, so ask first.
- If the browser's window is not listed (on another Space, or not on screen yet), `snapshot --surface web`, `find --surface web` and `open-url` still reach the page through the process `launch --cdp` started. Such a snapshot has no `data.window`.
- A browser started this way brings its own first window to the front once. `open-url`, snapshots and web ref actions after that leave the front app and the pointer where they are.
- `--user-data-dir` keeps the session out of the user's profile. While the DevTools port is open, any local process of the same user can drive the browser; `close-app "Google Chrome"` closes it.

## The loop

```bash
agent-desktop open-url "https://example.com/" --app "Google Chrome"
agent-desktop snapshot --app "Google Chrome" --surface web -i --lines
agent-desktop click @s8f3k2p9:e1
agent-desktop snapshot --app "Google Chrome" --surface web -i --lines
```

- `open-url` returns after the page's load event, with `url`, `title` and `opened` (`current_tab` or `new_tab`). `--new-tab` opens a background tab.
- Navigate with `open-url`. It does not raise the browser and returns after the load event.
- Read `data.channel` in the snapshot. `cdp` means refs act through the page, in the background: the window is not raised and the pointer does not move. `ax` means no endpoint reached the browser; it still works, and `data.suggestion` names the `launch --cdp` command.
- `--lines` prints the page as `data.lines`, one line per node, indented by depth: its ref first, then role and quoted name, then `value=`, `desc=` (when it differs from the name) and `[states]` when it has them, as in `@s8f3k2p9:e4 textfield "Email" value="ada@example.test"` or `@s8f3k2p9:e9 button "Menu" [expanded]`. Take the ref from the start of the line that names the control. It costs about a third fewer tokens than the JSON tree. Leave `--lines` out only to read a field it does not print, such as `available_actions` or `name_source`.
- `-i` keeps interactive nodes only. Leave it out to read page text such as totals, codes or table cells. On a long page, grep the lines instead of reading all of them: `agent-desktop snapshot --app "Google Chrome" --surface web --lines | grep -n "Total"`.
- Page text reads one block at a time. A paragraph, list item or caption that holds only text, links and buttons carries its whole text as its name, link text included; its links and buttons are its children, each with its own ref. Other text sits in `statictext` nodes, one per run between fields, images and blocks. `find --app "Google Chrome" --surface web --text "late 1980s"` finds the block that holds those words.
- A very long page can hold more refs than one snapshot stores (1 MB). The snapshot still succeeds, with `"complete": false` and `"refmap_capped": true`, and its tree stops after the last ref that fit. Every ref it shows is saved and works. For an element past the end, use `find --app "Google Chrome" --surface web --role link --name "History"` (any role and name) or add `-i`; a larger `--timeout-ms` does not help.
- `click` returns once the click is delivered. A snapshot taken right after it waits for the page a link or a submit opens, so take it at once, with no `sleep`. Check `data.url` in the new snapshot: a page that navigates later on its own, on a timer, can still show the old one, so snapshot again. A ref from the old page then fails with `STALE_REF`.
- A ref action with `-w 'role:text'` returns the snapshot once that element appears; `--wait-for-gone 'role:text'` returns `gone: true` once it goes, with no snapshot. Both read the page over the same endpoint, so browser UI outside the page, such as a JavaScript dialog, is not seen. Use it for a change that comes later than the action's own page load, such as suggestions under a search field after `press down --ref @field`.
- The page is the visible tab of the window you name. With several Chrome windows, pass `--window-id` from `list-windows`. `list-surfaces --app "Google Chrome"` lists each open page as a `web` surface.

## Acting on refs

| Goal | Command |
|---|---|
| Click a link or button, or submit a form | `click @ref` |
| Fill a text field, replacing its text | `set-value @ref "text"` |
| Set a date, time, month, week or color input | `set-value @ref "19:30"`, in the input's own format |
| Choose an option in a select, by label or value | `select @ref "Norway"` |
| Tick or untick a checkbox | `check @ref`, `uncheck @ref` |
| Choose a radio button | `check @ref` |
| Press Enter or another key in a field | `press return --ref @ref` |

A row, menu entry or card that the page makes clickable with a script, such as a `<div>` or `<span>` with a click listener and no ARIA role, is a `group` with a ref and `Click` in its `available_actions`. When the page gives it no accessible name, its text becomes its `name`, marked `"name_source": "contained_text"`, so `find --name` finds it and `click @ref` clicks it. A clickable element that holds a link, a button or an iframe keeps no ref of its own: click the control inside it. Neither does one holding more than 200 characters of text, such as a whole page section. An element whose click handler sits only on a parent element has no ref. Reach it through the page's keyboard path where one exists, such as `press down --ref @field` and then `press return --ref @field` in a field that opened a suggestion list.

`type`, `clear`, `toggle`, `focus`, `scroll`, `scroll-to` and `double-click` also work on web refs. Other actions return `ACTION_NOT_SUPPORTED`, and `--headed` is refused. To press a key in a page field, name the field: `press return --ref @ref` sends the key through the page, in the background. Web refs take single keys; a combo with a modifier returns `ACTION_NOT_SUPPORTED`. `press` without `--ref` is an OS key event that goes wherever the browser's keyboard focus is, often the address bar, so it can miss the field that `focus` chose.

### Date, time and color inputs

A date, time, datetime-local, month, week or color input is one ref with role `datefield`, `timefield` or `colorwell`. Its value is set whole with `set-value`, in the format the page itself reads:

| Input type | Role | Format | Example |
|---|---|---|---|
| `date` | `datefield` | `YYYY-MM-DD` | `2026-11-08` |
| `time` | `timefield` | `HH:MM`, 24-hour, optional `:SS` | `19:30` |
| `datetime-local` | `datefield` | `YYYY-MM-DDTHH:MM` | `2026-11-08T19:30` |
| `month` | `datefield` | `YYYY-MM` | `2026-11` |
| `week` | `datefield` | `YYYY-Www` | `2026-W45` |
| `color` | `colorwell` | `#rrggbb`, lowercase | `#ff8800` |

- `set-value` fires the page's `input` and `change` events and reads the value back. `delivered_verified` means the input holds exactly that value. `clear @ref` empties a date or time input.
- A value in another form (`7:30 PM`, `#FF8800`) returns `INVALID_ARGS`, and the suggestion starts with the exact value to pass: `Pass '19:30'.` A value that is not a value of the type (`11/08/2026`) returns `INVALID_ARGS` with the format.
- The `incrementor` refs under the field (Hours, Minutes, AM/PM, Day) are parts of it with no value of their own. `set-value` and `type` on them return `ACTION_NOT_SUPPORTED`. Set the field's own ref instead; do not step the parts with keys.
- `min`, `max` and `step` do not stop `set-value`: the browser keeps a value outside them, and the form flags it on submit.

Several steps on one page cost one turn as a `batch`:

```bash
agent-desktop batch '[
  {"command":"set-value","args":{"ref_id":"@s8f3k2p9:e4","value":"Ada Lovelace"}},
  {"command":"select","args":{"ref_id":"@s8f3k2p9:e6","value":"Norway"}},
  {"command":"check","args":{"ref_id":"@s8f3k2p9:e22"}},
  {"command":"click","args":{"ref_id":"@s8f3k2p9:e24"}}
]' --stop-on-error
```

Write every ref in full: a shell variable spliced into the JSON can make an agent's shell tool refuse the whole command.

A batch has `ok: true` even when an entry failed. When `data.failed_entries` and `data.not_started_entries` are both `0`, every entry worked. Otherwise read each entry's `ok` and `error` in `data.results`.

## Errors

| `error.code` and `details.kind` | Meaning | Do |
|---|---|---|
| `STALE_REF`, `cdp_document_replaced` or `cdp_node_gone` | The page navigated or the element went away since the snapshot | Snapshot again, use the new ref |
| `ACTION_FAILED`, `cdp_click_intercepted` | Something covers the element (a banner, a dialog) | Snapshot, dismiss the cover, retry |
| `ACTION_FAILED`, `cdp_node_not_rendered` | The element has no box on screen (hidden, collapsed) | Open the section that holds it, snapshot again |
| `WINDOW_NOT_FOUND`, message `No window found to identify the application owning surface 'web'`, right after a launch | The browser lists no window and no endpoint was recorded for it | Run `agent-desktop launch "Google Chrome" --activate` once, then retry |
| `WINDOW_NOT_FOUND`, `cdp_target_gone` | The tab closed | `list-surfaces --app "Google Chrome"` or `open-url` |
| `WINDOW_NOT_FOUND`, `cdp_browser_has_no_page` or `cdp_page_not_found` | No page to read (a fresh profile, a minimized window) | `open-url`, or restore the window |
| `AMBIGUOUS_TARGET`, `cdp_window_indistinct` | Two browser windows have the same title and frame, so the named window's page can't be told apart | Bring the window to the front or move one of them, then snapshot again |
| `ACTION_NOT_SUPPORTED`, `cdp_not_a_browser` | `open-url` on an app that embeds web content for its own interface (an Electron app) | Read and act with `snapshot --surface web`; open URLs in a web browser |
| `INVALID_ARGS`, `cdp_value_not_canonical` or `cdp_value_malformed` | The value is not in the input's own format | Pass the value the suggestion names, or the format it gives |
| `ACTION_FAILED`, `cdp_value_rejected` | The input refused the value, such as `2026-02-30`, and kept its old one | Pass a value that exists |
| `ACTION_FAILED`, `cdp_value_adjusted` | The input or the page changed the value after it was set; `details.observed` holds it | Read it with `get @ref --property value` before you set it again |
| `ACTION_NOT_SUPPORTED`, message `role incrementor is not editable` | The ref is one part of a date or time input, or a stepper | `set-value` on the field that contains it |
| `PERM_DENIED`, `cdp_approval_required` | Chrome asked its user to allow the connection | Ask the user; do not retry in a loop |

Retry an action only when `error.disposition.retry` is `safe`. Otherwise read the page again and decide from what it shows.
