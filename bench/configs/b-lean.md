
## Observation budget

Every byte of agent-desktop output is re-read on every later turn, so keep observations small:
- Prefer `agent-desktop find --app <App> --role <role> --name <name>` when you know what you are looking for.
- When you must snapshot, always pass `-i --compact`. On a large window start with `--skeleton -i --compact` and drill with `--root <ref> -i --compact`.
- Never take a snapshot without `-i` unless you need static text that `-i` drops; then drill only the region that holds it.
