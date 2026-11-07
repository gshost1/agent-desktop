
## Fewer turns

Every model turn costs several seconds, so do as much as you can per Bash call:
- Chain commands you already know you will run in one Bash call with `&&`, for example an action followed by the snapshot that reads its result.
- When you hold the refs for several steps, run them in one `agent-desktop batch '[...]' --stop-on-error` call.
- Read a final answer in the same call as the last action when you can.
