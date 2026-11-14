# Shell syntax and fewer round trips

Each tool call is a turn, and every turn re-reads everything before it. These forms do the work of several calls in one.

## Shell syntax

Examples use a POSIX shell. When a ref is built from a variable, brace it and keep the `e` in each part you list: `for r in e7 e8; do agent-desktop click "${S}:${r}"; done`. In zsh, `$S:e1` applies the `:e` modifier and passes `1`, even inside double quotes. A list of bare numbers (`for n in 7 8` with `"${S}:e${n}"`) is easy to mix up with the digits or values you are typing. In PowerShell, quote every ref (`'@s8f3k2p9:e1'`), because a bare `@token` is the splatting operator. Use `$env:NAME = 'value'` instead of `export`, and `| ConvertFrom-Json` instead of `$(...)` and `jq`. The Windows skill has the PowerShell forms.

## Chain calls

```bash
agent-desktop launch "Font Book" && agent-desktop snapshot --app "Font Book" -i --compact --lines
R=$(agent-desktop find --app "Font Book" --role button --name "Hide Sidebar" | jq -r '.data.matches[0].ref_id')
agent-desktop click "${R}" -w "button:Show Sidebar"
agent-desktop wait --app "Font Book" --text "Show Sidebar" --timeout 5000
```

- Chain `launch` and the first snapshot in one call.
- `find` the element you will act on, by its own name. The pane that holds it is often open already, so finding the parent first costs a turn.
- Never cut a snapshot or a `-w` result with `head` or `head -c`: the cut drops refs and the text you need, which is often near the end, and you read it again. Narrow it with `-i`, `--root <ref>`, `find`, `grep`, or `jq` over the whole output.
- After an action, `-w "role:text"` (or `-w ":text"`) polls until the effect shows and returns the snapshot, so the read is not a separate call. A separate `wait` takes `--timeout` in ms.
- When the task names the path (English, then Arial), take the whole path and the answer in one call: `find` each step by its name, `click` it with `-w` naming the next step, and end with a read filtered by `grep` for what the task asks. Do not stop between steps to read refs: each step's ref comes from its own `find`, and `jq -e` stops the `&&` chain at the first missing match or failed step:

```bash
R=$(agent-desktop find --app "Font Book" --role cell --name "English" --exact | jq -er '.data.matches[0].ref_id') &&
agent-desktop click "${R}" -w ":Show Arial" | jq -e '.ok' >/dev/null &&
R=$(agent-desktop find --app "Font Book" --name "Show Arial" --exact | jq -er '.data.matches[0].ref_id') &&
agent-desktop click "${R}" -w "button:Go back" --lines | jq -r '.data.lines[]' | grep -i "arial"
```
