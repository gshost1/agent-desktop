---
name: agent-desktop-web
tags: browser-automation, web, cdp, chrome, ai-agent, cli
requirements:
  - agent-desktop
description: >
  Read and operate web pages in Chrome with agent-desktop's snapshot and ref loop, over the
  browser's DevTools endpoint. Use for any task on a web page: open a URL, read a page, fill
  and submit a form, click links, pick options.
  Triggers on: "open the page", "fill the form", "web page", "in Chrome", "browser".
---

# agent-desktop: web pages

Every command prints one JSON envelope: read `ok`, then `data` or `error`. This quickstart covers the loop. Launch options, date and time inputs, several windows, long pages and every error code are in `agent-desktop skills get web reference`. For a native app in the same task, also read `agent-desktop skills get desktop`.

Chrome needs a DevTools endpoint. Skip this when the task says Chrome is already running with one:

```bash
agent-desktop launch "Google Chrome" --cdp --arg=--user-data-dir="$PWD/chrome-profile" --arg=--no-first-run --arg=--no-default-browser-check
```

## The loop

```bash
agent-desktop open-url "https://example.com/" --app "Google Chrome"
agent-desktop snapshot --app "Google Chrome" --surface web -i --lines
agent-desktop click @s8f3k2p9:e1
agent-desktop snapshot --app "Google Chrome" --surface web -i --lines
```

- `data.lines` holds one line per node, indented by depth: ref, role, quoted name, then `value=`, `desc=` and `[states]`. Copy the ref from the start of the line that names the control. Each snapshot gives new refs.
- Take the snapshot right after a click, with no `sleep`: it waits for the page the click opens. Its `data.url` and `data.title` name the page.
- `-i` keeps interactive nodes only. To read text such as a total, a code or a sentence, search for it instead of reading the whole page:

```bash
agent-desktop find --app "Google Chrome" --surface web --text "late 1980s"
agent-desktop snapshot --app "Google Chrome" --surface web --lines | grep -n "Total"
```

- `find --text` also returns the containers around the text. Read the match whose `name` holds the words: a paragraph or list item carries its whole text as its name.

## Acting on refs

| Goal | Command |
|---|---|
| Click a link or button | `click @ref` |
| Fill a text field, replacing its text | `set-value @ref "text"` |
| Set a date, time or color input | `set-value @ref "2026-11-08"`, `"19:30"`, `"#ff8800"` |
| Choose a select option, by label or value | `select @ref "Norway"` |
| Tick a checkbox or choose a radio button | `check @ref` (`uncheck @ref` to clear) |
| Press Enter in a field | `press return --ref @ref` |

`data.disposition.delivery` `delivered_verified` means the element was read back and holds the value, as `data.post_state` shows. Do not set it again. `application_commit: not_verified` only says the page's own save was not observed.

Several steps on one page fit in one `batch`:

```bash
agent-desktop batch '[
  {"command":"set-value","args":{"ref_id":"@s8f3k2p9:e4","value":"Ada Lovelace"}},
  {"command":"select","args":{"ref_id":"@s8f3k2p9:e6","value":"Norway"}},
  {"command":"check","args":{"ref_id":"@s8f3k2p9:e22"}}
]' --stop-on-error
```

A batch has `ok: true` even when an entry failed. `data.failed_entries` is `0` when every entry worked; then do not repeat them. Otherwise read the failed entry's `error` in `data.results`.

## Errors

- `STALE_REF`: the page changed since the snapshot. Snapshot again and use the new ref.
- `ACTION_FAILED` with `details.kind` `cdp_click_intercepted`: a banner or dialog covers the element. Dismiss it, then retry.
- Retry an action only when `error.disposition.retry` is `safe`. For any other code, see `agent-desktop skills get web reference`.
