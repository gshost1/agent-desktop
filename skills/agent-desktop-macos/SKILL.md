---
name: agent-desktop-macos
tags: macos-automation, accessibility, ax, tcc, ai-agent, gui-automation, cli
requirements:
  - agent-desktop
description: >
  macOS platform guide for the agent-desktop CLI. Use for desktop automation
  on macOS: selecting text by content, formatting and saving documents,
  floating panels such as Fonts, linked fields in sheets, key chords and menu
  shortcuts, headed mode, permissions (TCC), surfaces, and fixing PERM_DENIED,
  empty trees, ACTION_FAILED and APP_UNRESPONSIVE.
---

# agent-desktop on macOS

Read the core skill first (`agent-desktop skills get desktop`). This page has the macOS recipes agents need most. The full rules are in three references, each read with `agent-desktop skills get platform <topic>`:

- `actions`: headed actions, scroll into view, in-place edits, menus, keys, menu shortcuts, Return and Escape on sheets.
- `apps-and-windows`: requirements and permissions, surfaces, file lists, long lists in `find`, Chromium, launch, identity, refs.
- `troubleshooting`: one entry per symptom or error code.

## Shell

- Write every flag out in full on each call. In zsh, `A="--app TextEdit"; agent-desktop press $A cmd+b` passes `--app TextEdit` as one word, so the call fails with a parse error.
- Never send stderr to `/dev/null` and never discard the JSON: read `ok` and `error.code` on every step.

## Setup

Run `agent-desktop permissions`. If Accessibility is denied, run `agent-desktop permissions --guide`, which opens System Settings for the user. Screen Recording is needed only for `screenshot`. The primary modifier is `cmd`: `press cmd+s --app TextEdit`.

## Select text by its content

`select @<textfield> "<text>"` selects that exact text, case included, in a text field or text area, and reads the selection back. Use it for a word, a phrase or a whole paragraph: pass the paragraph's text word for word from the snapshot's `value`. Do not select a paragraph with `cmd+down` then `shift+up`; that takes only its last wrapped line.

- Text that occurs more than once fails `AMBIGUOUS_TARGET` with `details.offsets`, and nothing changes: add neighbouring words.
- Text that is absent fails `ELEMENT_NOT_FOUND`: read the value again with `get @ref --property value`.
- For a few characters inside a repeated word, select a run that occurs once, `press left --app <App>` to put the caret at its start, then `press right` and `press shift+right` as many times as needed.

## Format the selection, then save

1. Click the format's menu item: `find --app <App> --surface menubar --name "<Item>" --exact`, then `click` the ref. Always pass `--exact` here: once a format is applied, Edit > `Undo <Item>` also contains the name, and clicking it undoes your work. A shortcut such as `press cmd+b --app <App>` also runs it. Repeat steps from `select` for each occurrence, then save once.
2. A format with no menu item, such as strikethrough in TextEdit, is in the Fonts panel. Open it with `press cmd+t --app <App>`, which toggles the panel, so check `list-windows` first. Then follow Floating panels below. In the panel, `select` `Single` on its strikethrough pop-up, or `click` the Font Attributes menu button and then `click` the `Strikethrough` item in `--surface menu`. Click that item headless: a headed `click` on a pop-up menu item outside the menu bar fails `ACTION_FAILED` before any input.
3. Save with `press cmd+s --app <App>`. The step `AXPress menu shortcut` with `data.details.menu_shortcut.item: "Save"` ran File > Save, which writes a document that already has a file in place.

```bash
agent-desktop snapshot --app TextEdit -i --compact --lines
agent-desktop select @s8f3k2p9:e2 "H2O and"
agent-desktop press left --app TextEdit
agent-desktop press right --app TextEdit
agent-desktop press shift+right --app TextEdit
agent-desktop find --app TextEdit --surface menubar --name "Subscript" --exact
agent-desktop click @s3k9d2m1:e1
agent-desktop press cmd+s --app TextEdit
```

A format in the Fonts panel:

```bash
agent-desktop select @s8f3k2p9:e2 "The final paragraph, word for word."
agent-desktop press cmd+t --app TextEdit
agent-desktop list-windows --app TextEdit
agent-desktop snapshot --window-id w-4521 -i --compact --lines
```

## Floating panels

A panel that floats above an app's documents, such as Fonts, Colors or an inspector, is its own window. While it is on screen, `list-windows --app <App>` lists it by title with its own id. Read it with `snapshot --window-id <id> -i` or `find --window-id <id>`; its refs act in the panel, headed or not. `--app <App>` still reads the document window, also while a text field's suggestion list is open. A panel usually hides while its app is in the background, and a hidden panel is not listed.

## Long lists

A snapshot prints a long list or grid before the toolbar and search field below it, so read all of it and never cut it with `head`. To reach one item of a long list, `set-value` its name into the window's `textfield "Search"`, snapshot again, and read the filtered list instead of scrolling for it.

## Wait for the effect in the same call

Add `-w ":<text>"` or `-w "<role>:<text>"` to a ref action: it polls until the text shows and returns the snapshot, so you need no `sleep` and no second read. For example, `agent-desktop click @s8f3k2p9:e5 -w ":Time zone" --lines`. Never cut that output with `head` or `tail`; filter it with `grep` or `jq`.

## Linked fields in sheets

`set-value @ref "<value>"` on a text field gives it keyboard focus in its window first, without raising the app, so the app's own edit handling runs. A plain single-line text field that `set-value` writes returns `delivered_unverified` with `application_commit: "unobserved"`: the field holds the text, but many apps take it into the document only when editing ends. Before you save or confirm, run `focus` on another field in the same window, then check the result. Do not press Return in such a field to commit it: Return can submit a form or press a default button. A text area also reports `unobserved`: `focus` elsewhere commits some text views and not others, so after you save, check the saved result. A search field, secure field or pop-up is settled by the read-back and needs no extra step. In an image-size sheet that keeps the aspect ratio, setting the height updates the width. Read the linked field back before you confirm the sheet. Its fields may have no names: tell them apart by order and value, and check the unit pop-up (`select @ref "pixels"`) before you write numbers.

## Keys and menu shortcuts

- With `cmd` and `--app`, `press` first looks for a menu item with that shortcut and presses it with `AXPress`, headless included: `data.details.menu_shortcut` names the `menu` and `item`.
- If no enabled item matches and the app is in the background, a character chord such as `cmd+b` fails `ACTION_FAILED` `command_chord_unrouted_in_background`, and nothing is sent. Rerun with `--headed`, or `click` the menu item. Arrows and navigation chords still reach a background app's focused text view.
- A disabled match in a background app fails with `menu_item_disabled_in_background`. Rerun with `--headed`.
- Quit chords return `POLICY_DENIED`. Use `close-app`.

## Headed or headless

Headless is the default and acts through accessibility: `click`, `select` and `set-value` work on a background app. `--headed` brings the window forward and uses the real pointer and keyboard. `double-click`, `triple-click`, `hover` and `drag` need it. Check every effect by observation: a success code does not prove an effect, so snapshot again or `get` the value.

## Sheets and menus

`press return --app <App>` or `escape` on a sheet checks that the sheet closed. If it fails with `effect: "none_observed"`, do not press again: snapshot `--surface sheet -i` and click its default button. For an app's menus, use `--surface menubar`. For an open context or pop-up menu, use `--surface menu`. For a combo box or pop-up button, use `select @ref "Option"`.
