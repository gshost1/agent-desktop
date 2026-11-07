
## Observation budget

Every byte of agent-desktop output is re-read on every later turn, so keep observations small:
- Prefer `agent-desktop find --app <App> --role <role> --name <name>` when you know what you are looking for.
- When you must snapshot, always pass `-i --compact`. On a large window start with `--skeleton -i --compact` and drill with `--root <ref> -i --compact`.
- Never take a snapshot without `-i` unless you need static text that `-i` drops; then drill only the region that holds it.

## Fewer turns

Every model turn costs several seconds, so do as much as you can per Bash call:
- Chain commands you already know you will run in one Bash call with `&&`, for example an action followed by the snapshot that reads its result.
- When you hold the refs for several steps, run them in one `agent-desktop batch '[...]' --stop-on-error` call.
- Read a final answer in the same call as the last action when you can.

## Re-observe only when needed

Do not take a snapshot after an action just to confirm it. An action result with `ok: true` landed; check its `data.surfaces` for a new menu, sheet or dialog. Re-observe only when the next step needs refs you do not hold, when the result reports an error or a surface, or once at the end to read or confirm the outcome.
