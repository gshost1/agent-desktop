# Held-out arms

Every arm runs through `claude -p` on the subscription with Bash only. Each arm gets the same task prompt; only the tool paragraph of the system prompt differs (`common.py`, `run_web.py`). That paragraph tells the model where the tool's own guide is, and the model learns the tool from there. Every tool call goes through `bin/<arm>/<tool>`, which logs its wall time to the run's tool log (`logwrap.zsh`). Third-party tools are installed only under `~/.agent-desktop-bench-target/heldout-tools/`, at exact versions, from release artifacts whose checksums were verified on 2026-11-10.

| Arm | Suite | Tool and version | Guide the model reads | Status |
|---|---|---|---|---|
| `ad-14` | native, web (`ad-web` channel) | agent-desktop 1.4.0, fork release `v1.4.0` (`chore: release 1.4.0`, e7c15ec6e; bench-main14 697c618d6 is its ancestor) | `agent-desktop skills get desktop` / `platform`; web: `skills get web` | ready |
| `ad-up` | native, web (`ax` channel; upstream has no web channel) | agent-desktop upstream 8e8dd7cbe, the existing `bench-upstream` release build | `agent-desktop skills get desktop` / `platform` | ready |
| `bh` | web (`cdp` channel) | browser-harness 0.1.13 (PyPI), the existing `bh-venv` | `browser-harness skill` | ready; the 1110-evening web runs already used this setup |
| `pw` | web (`cdp` channel) | @playwright/cli 0.1.24 (microsoft/playwright-cli) | `cat …/skills/playwright-cli/SKILL.md` | ready; installed and `--help` checked; never driven yet, so the first round doubles as its smoke run |
| `peekaboo` | native | Peekaboo CLI 4.9.8 (openclaw/Peekaboo) | `cat …/skills/peekaboo/SKILL.md` (tag v4.9.8) | needs preflight, see below |
| `cua` | native | Cua Driver 0.46.1 (trycua/cua `cua-driver-rs-v0.46.1`) | `cat …/skills/cua-driver/SKILL.md` and its other guides (tag `cua-driver-rs-v0.46.1`) | **blocked on the Mac**, see below; ready on AWS |

## Install record

All commands ran on the Mac on 2026-11-10. They install nothing outside `heldout-tools/`, change no TCC grant, and run no GUI.

```bash
T=$HOME/.agent-desktop-bench-target/heldout-tools
# agent-desktop 1.4.0
mkdir -p $T/downloads/agent-desktop-1.4.0 $T/agent-desktop-1.4.0 && cd $T/downloads/agent-desktop-1.4.0
gh release download v1.4.0 --repo gshost1/agent-desktop -p 'agent-desktop-v1.4.0-aarch64-apple-darwin.tar.gz' -p checksums.txt
grep 'agent-desktop-v1.4.0-aarch64-apple-darwin' checksums.txt | shasum -a 256 -c   # 87bd1ff0…d0f36a OK
tar -xzf agent-desktop-v1.4.0-aarch64-apple-darwin.tar.gz -C $T/agent-desktop-1.4.0
# Peekaboo 4.9.8
mkdir -p $T/downloads/peekaboo-4.9.8 $T/peekaboo-4.9.8 && cd $T/downloads/peekaboo-4.9.8
curl -fsSLO https://github.com/openclaw/Peekaboo/releases/download/v4.9.8/checksums.txt
curl -fsSLO https://github.com/openclaw/Peekaboo/releases/download/v4.9.8/peekaboo-macos-arm64.tar.gz
grep peekaboo-macos-arm64.tar.gz checksums.txt | shasum -a 256 -c                  # 5b82b246…685bfe OK
tar -xzf peekaboo-macos-arm64.tar.gz -C $T/peekaboo-4.9.8
gh api 'repos/openclaw/Peekaboo/contents/skills/peekaboo/SKILL.md?ref=v4.9.8' --jq .content | base64 -d > $T/skills/peekaboo/SKILL.md
# Cua Driver 0.46.1
mkdir -p $T/downloads/cua-driver-0.46.1 $T/cua-driver-0.46.1 && cd $T/downloads/cua-driver-0.46.1
curl -fsSLO https://github.com/trycua/cua/releases/download/cua-driver-rs-v0.46.1/checksums.txt
curl -fsSLO https://github.com/trycua/cua/releases/download/cua-driver-rs-v0.46.1/cua-driver-rs-0.46.1-darwin-arm64.tar.gz
grep 'darwin-arm64.tar.gz$' checksums.txt | shasum -a 256 -c                       # f7f226af…847766 OK
tar -xzf cua-driver-rs-0.46.1-darwin-arm64.tar.gz -C $T/cua-driver-0.46.1
for f in SKILL.md MACOS.md WORKFLOW.md RUNTIME.md README.md; do
  gh api "repos/trycua/cua/contents/libs/cua-driver/rust/Skills/cua-driver/$f?ref=cua-driver-rs-v0.46.1" --jq .content | base64 -d > $T/skills/cua-driver/$f
done
# Playwright CLI 0.1.24 (no install scripts, no browser download; it attaches to the bench Chrome)
mkdir -p $T/playwright-cli-0.1.24 && PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 \
  npm install --prefix $T/playwright-cli-0.1.24 --ignore-scripts --no-audit --no-fund --save-exact @playwright/cli@0.1.24
cp -R $T/playwright-cli-0.1.24/node_modules/@playwright/cli/skills/playwright-cli/{SKILL.md,references} $T/skills/playwright-cli/
# browser-harness 0.1.13: reused, not reinstalled. Recreate elsewhere (AWS) with:
#   uv venv --python 3.12 ~/.agent-desktop-bench-target/bh-venv && uv pip install --python ~/.agent-desktop-bench-target/bh-venv/bin/python browser-harness==0.1.13
```

## Telemetry and updates

| Tool | Setting in the wrapper | Source |
|---|---|---|
| Cua Driver | `CUA_DRIVER_RS_TELEMETRY_ENABLED=0`, `CUA_TELEMETRY_ENABLED=false`, private `CUA_DRIVER_RS_HOME`; the wrapper refuses `update`, `autostart` and `telemetry` | `libs/cua-driver/rust/crates/cua-driver/src/telemetry.rs` at the tag (PostHog capture, env override first) |
| Playwright CLI | `NO_UPDATE_NOTIFIER=1` (its only phone-home is an `npm view` update check) | `@playwright/cli/playwright-cli.js` |
| browser-harness | `BH_TELEMETRY=0 BH_UPDATE_CHECK=0 ANONYMIZED_TELEMETRY=false` | same as the 1110-evening wrapper |
| Peekaboo | `PEEKABOO_AI_PROVIDERS=""` (no model calls), private `PEEKABOO_CONFIG_DIR` and daemon socket; no telemetry is documented | `docs/configuration.md`, `docs/security.md` at v4.9.8 |
| agent-desktop | none needed | |

## Wrapper rules (the same kind for every arm)

- `peekaboo agent` is refused: it runs its own model, and the comparison is about the tool, not a second agent.
- Whole-screen and whole-desktop reads are refused (`peekaboo … --mode screen|multi`, `cua-driver call get_desktop_state`). They would read windows outside the bench. agent-desktop has no whole-screen tree read, and screenshots fail for every arm on the Mac because Screen Recording is off.
- `cat` is allowed only for the arm's own guide folder (native audit) and for `.playwright-cli/` snapshot files (web; `cat` is allowed for every web arm alike).
- Any other GUI tool on PATH (agent-desktop, peekaboo, cua-driver, browser-harness, playwright-cli, agent-browser) fails the run if the arm calls it.

## Why peekaboo needs a preflight

The pinned CLI is signed by `FWJYW4S8P8` with identifier `boo.peekaboo.peekaboo`. Peekaboo's own docs say macOS treats its signed CLI as its own TCC client. No Peekaboo Bridge socket exists on the Mac (none of the three documented sockets is present), so the wrapper's per-run daemon or the local process does the work. Not confirmed: whether macOS charges its Accessibility use to the terminal that runs the bench, as it does for agent-desktop, or to the CLI itself. `preflight.sh` prints `peekaboo permissions status --all-sources --json`; its docs say that path never prompts. If it reports Accessibility missing, George grants Accessibility to `~/.agent-desktop-bench-target/heldout-tools/peekaboo-4.9.8/peekaboo-macos-arm64/peekaboo`, or the arm is dropped on the Mac.

## Why cua is blocked on the Mac

Cua Driver 0.46.1 needs Accessibility for its own app identity, `com.trycua.driver` (team `YCK386LBJ7`). Its macOS guide says the CLI relaunches "the installed CuaDriver.app by its exact path" when it is started from an IDE-spawned shell such as Claude Code, so that TCC attributes the process to the app. The pinned app sits in `heldout-tools/`, not `/Applications`. Not confirmed: whether the CLI finds an app outside `/Applications`. Two things are George's call: a TCC grant, and possibly an install outside `heldout-tools`. Options:
1. George grants Accessibility to `heldout-tools/cua-driver-0.46.1/cua-driver-rs-0.46.1-darwin-arm64/CuaDriver.app`. Preflight (`cua-driver permissions status --json`, read-only) then shows whether the CLI uses it. If it insists on `/Applications`, George approves copying the app there.
2. Run the cua arm on the AWS host only (`aws_setup.md`). There the grant is part of the golden image, and nothing personal is on screen. **Recommended.**
