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

Every command prints one JSON envelope: read `ok`, then `data` or `error`. Date and time inputs, several windows, long pages and every error code are in `agent-desktop skills get web reference`. For a native app in the same task, also read `agent-desktop skills get desktop`.

Chrome needs a DevTools endpoint. Skip this when the task says Chrome already has one:

```bash
agent-desktop launch "Google Chrome" --cdp --arg=--user-data-dir="$PWD/chrome-profile" --arg=--no-first-run --arg=--no-default-browser-check
```

If Chrome is already running, `launch` fails with `cdp_requires_fresh_launch`. Closing it loses the user's tabs: ask first.

## The loop

```bash
agent-desktop open-url "https://example.com/" --app "Google Chrome"
agent-desktop snapshot --app "Google Chrome" --surface web -i --lines
agent-desktop click @s8f3k2p9:e1
agent-desktop snapshot --app "Google Chrome" --surface web -i --lines
```

- `data.channel` is `cdp`. With `ax`, no endpoint was reached: follow `data.suggestion`.
- `data.lines` holds one line per node, indented by depth: ref, role, quoted name, then `value=`, `desc=` (when it differs from the name), `placeholder=` and `[states]`. Copy the ref from the line that names the control. Each snapshot gives new refs.
- Snapshot right after a click, with no `sleep`: it waits for the page the click opens. `data.url` names the page. For a slower change, add `-w 'role:text'` to the action to get the snapshot once that appears.
- A search field's suggestions appear after a key: `press down --ref @field -w 'statictext:Honolulu'`, then `press down`, `press return --ref @field`.
- `-i` keeps interactive nodes only. To read text such as a total or a sentence, search for it:

```bash
agent-desktop find --app "Google Chrome" --surface web --text "late 1980s"
agent-desktop snapshot --app "Google Chrome" --surface web --lines | grep -nE '"complete"|Total'
```

- `find --text` also returns the containers around the text. Read the match whose `name` holds the words.
- `"complete": false` means a long page was cut off; use `find --text` for what is past the end.

## Acting on refs

| Goal | Command |
|---|---|
| Click a link or button | `click @ref` |
| Fill a text field, replacing its text | `set-value @ref "text"` |
| Set a date, time or color input | `set-value @ref "2026-11-08"`, `"19:30"`, `"#ff8800"` |
| Choose a select option | `select @ref "Norway"` |
| Tick a checkbox or radio button | `check @ref` (`uncheck` clears) |
| Press Enter in a field | `press return --ref @ref` |

`delivered_verified` means the element was read back and holds the value: do not set it again, even with `application_commit: not_verified`.

Several steps on one page fit in one `batch`:

```bash
agent-desktop batch '[
  {"command":"set-value","args":{"ref_id":"@s8f3k2p9:e4","value":"Ada Lovelace"}},
  {"command":"check","args":{"ref_id":"@s8f3k2p9:e22"}}
]' --stop-on-error
```

Write every ref in full. A shell variable spliced into the JSON (`'"$P"':e4`) can make an agent's shell tool refuse the whole command.

A batch has `ok: true` even when an entry failed. When `data.failed_entries` and `data.not_started_entries` are both `0`, every entry worked: do not repeat them. Otherwise read each entry's `ok` and `error` in `data.results`.

## Errors

- `STALE_REF`: the page changed since the snapshot. Snapshot again and use the new ref.
- `ACTION_FAILED`, `cdp_click_intercepted`: a banner or dialog covers the element. Dismiss it, then retry.
- Retry only when `error.disposition.retry` is `safe`. For other codes, see `agent-desktop skills get web reference`.
