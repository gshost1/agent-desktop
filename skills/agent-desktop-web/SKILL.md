---
name: agent-desktop-web
tags: browser-automation, web, cdp, chrome, ai-agent, cli
requirements:
  - agent-desktop
description: >
  Read and operate web pages in Chrome with agent-desktop's snapshot and ref loop, in the
  background, over the browser's DevTools endpoint. Use for any task on a web page: open a URL,
  read a page, fill and submit a form, click links, pick options.
  Triggers on: "open the page", "fill the form", "web page", "in Chrome", "browser".
---

# agent-desktop: web pages

Every command prints one JSON envelope (`ok`, then `data` or `error`). A page in Chrome takes the same snapshot, ref and action loop as a native app. This skill is enough for web work. For native apps in the same task (Finder, TextEdit, a Save panel), also read `agent-desktop skills get desktop`.

## Start the browser

```bash
agent-desktop launch "Google Chrome" --cdp --arg=--user-data-dir="$PWD/chrome-profile" --arg=--no-first-run --arg=--no-default-browser-check
```

- `--cdp` needs a fresh browser process. If Chrome is already running, `launch` returns `ACTION_FAILED` with `details.kind` `cdp_requires_fresh_launch`. Closing the user's Chrome loses their tabs, so ask first.
- A browser started this way brings its own first window to the front once. `open-url`, snapshots and web ref actions after that leave the front app and the pointer where they are.
- `--user-data-dir` keeps the session out of the user's profile. While the DevTools port is open, any local process of the same user can drive the browser; `close-app "Google Chrome"` closes it.

## The loop

```bash
agent-desktop open-url "https://example.com/" --app "Google Chrome"
agent-desktop snapshot --app "Google Chrome" --surface web -i
agent-desktop click @s8f3k2p9:e1
agent-desktop snapshot --app "Google Chrome" --surface web -i
```

- `open-url` returns after the page's load event, with `url`, `title` and `opened` (`current_tab` or `new_tab`). `--new-tab` opens a background tab.
- Navigate with `open-url`. It does not raise the browser and returns after the load event.
- Read `data.channel` in the snapshot. `cdp` means refs act through the page, in the background: the window is not raised and the pointer does not move. `ax` means no endpoint reached the browser; it still works, and `data.suggestion` names the `launch --cdp` command.
- `-i` keeps interactive nodes only. Leave it out to read page text such as totals, codes or table cells. On a long page, pipe the JSON through `jq` or `grep` instead of reading all of it.
- `click` returns once the click is delivered. A snapshot taken right after it waits for the page a link or a submit opens, so take it at once, with no `sleep`. Check `data.url` in the new snapshot: a page that navigates later on its own, on a timer, can still show the old one, so snapshot again. A ref from the old page then fails with `STALE_REF`.
- The page is the visible tab of the window you name. With several Chrome windows, pass `--window-id` from `list-windows`. `list-surfaces --app "Google Chrome"` lists each open page as a `web` surface.

## Acting on refs

| Goal | Command |
|---|---|
| Click a link or button, or submit a form | `click @ref` |
| Fill a text field, replacing its text | `set-value @ref "text"` |
| Choose an option in a select, by label or value | `select @ref "Norway"` |
| Tick or untick a checkbox | `check @ref`, `uncheck @ref` |
| Choose a radio button | `check @ref` |
| Press Enter or another key in a field | `press return --ref @ref` |

`type`, `clear`, `toggle`, `focus`, `scroll`, `scroll-to` and `double-click` also work on web refs. Other actions return `ACTION_NOT_SUPPORTED`, and `--headed` is refused. To press a key in a page field, name the field: `press return --ref @ref` sends the key through the page, in the background. Web refs take single keys; a combo with a modifier returns `ACTION_NOT_SUPPORTED`. `press` without `--ref` is an OS key event that goes wherever the browser's keyboard focus is, often the address bar, so it can miss the field that `focus` chose.

Several steps on one page cost one turn as a `batch`:

```bash
agent-desktop batch '[
  {"command":"set-value","args":{"ref_id":"@s8f3k2p9:e4","value":"Ada Lovelace"}},
  {"command":"select","args":{"ref_id":"@s8f3k2p9:e6","value":"Norway"}},
  {"command":"check","args":{"ref_id":"@s8f3k2p9:e22"}},
  {"command":"click","args":{"ref_id":"@s8f3k2p9:e24"}}
]' --stop-on-error
```

## Errors

| `error.code` and `details.kind` | Meaning | Do |
|---|---|---|
| `STALE_REF`, `cdp_document_replaced` or `cdp_node_gone` | The page navigated or the element went away since the snapshot | Snapshot again, use the new ref |
| `ACTION_FAILED`, `cdp_click_intercepted` | Something covers the element (a banner, a dialog) | Snapshot, dismiss the cover, retry |
| `ACTION_FAILED`, `cdp_node_not_rendered` | The element has no box on screen (hidden, collapsed) | Open the section that holds it, snapshot again |
| `WINDOW_NOT_FOUND`, `cdp_target_gone` | The tab closed | `list-surfaces --app "Google Chrome"` or `open-url` |
| `WINDOW_NOT_FOUND`, `cdp_browser_has_no_page` or `cdp_page_not_found` | No page to read (a fresh profile, a minimized window) | `open-url`, or restore the window |
| `AMBIGUOUS_TARGET`, `cdp_window_indistinct` | Two browser windows have the same title and frame, so the named window's page can't be told apart | Bring the window to the front or move one of them, then snapshot again |
| `ACTION_NOT_SUPPORTED`, `cdp_not_a_browser` | `open-url` on an app that embeds web content for its own interface (an Electron app) | Read and act with `snapshot --surface web`; open URLs in a web browser |
| `PERM_DENIED`, `cdp_approval_required` | Chrome asked its user to allow the connection | Ask the user; do not retry in a loop |

Retry an action only when `error.disposition.retry` is `safe`. Otherwise read the page again and decide from what it shows.
