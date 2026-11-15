# Releasing

Update the Cargo workspace version, lockfile, npm source version, and changelog in one pull request.

## Changelog fragments

Every pull request with a user-visible change, except the release pull request, adds one fragment and leaves
`CHANGELOG.md` alone. The fragment is `changelog.d/<branch>.md`, named after its branch with `/` replaced by `-`
(`fix/click-wait` writes `changelog.d/fix-click-wait.md`), and sits directly in `changelog.d/`:

```markdown
### Bug Fixes

* What changed, for the agent that calls agent-desktop, with measured numbers. ([#140](https://github.com/gshost1/agent-desktop/pull/140))
```

A fragment holds one or more `### <Heading>` sections. The headings are `BREAKING CHANGES`, `Features`, `Bug Fixes`,
`Performance`, `Documentation`, `Testing` and `Known issues`. Each heading has a blank line after it and one or more
single-line `* ` bullets, and the file ends with a newline. `python3 scripts/changelog_fragments.py check` checks every
fragment. The pre-commit hook checks staged fragments, refuses one in a subfolder, and refuses a staged `CHANGELOG.md`
unless the same commit stages the version bump in `Cargo.toml` or the fragment deletions that `assemble` makes. The
Format job in CI runs the fragment self-test and `check` on every pull request.

The release pull request assembles the fragments after it bumps the version:

```bash
python3 scripts/changelog_fragments.py assemble --version 1.5.0 --dry-run
python3 scripts/changelog_fragments.py assemble --version 1.5.0
```

`assemble` writes a `## [1.5.0](compare link) (date)` section at the top of `CHANGELOG.md`, with the headings in the
order above and the bullets of each heading ordered by pull request number. It copies every bullet byte for byte and
deletes the fragments it used; `changelog.d/.gitkeep` stays. The compare link runs from the previous top release, or
from `--previous`. `--date` defaults to today, and `--intro "<paragraph>"` adds a paragraph under the heading. It
refuses a version that already has a section, an empty `changelog.d/` and a malformed fragment, and then changes
nothing. The release pull request can assemble and bump in one commit or in two; each commit passes the hook.
`scripts/check-release-consistency.sh` runs the self-test and `check` too, and fails while the top section of
`CHANGELOG.md` is not the workspace version. The Release workflow runs it before it tags, so a version bump without
an assembled changelog stops the release there. Run it locally before you open the release pull request.

## Publishing

After it merges to `main`, changes to `Cargo.toml` trigger the existing Release workflow.
An unchanged published version skips publication. Manual dispatch remains available for retries.

The workflow builds and checks the native CLI and FFI archives, bundles the CLI executables into npm,
publishes npm through trusted publishing (OIDC), and publishes GitHub archives, checksums, and provenance
with the built-in `GITHUB_TOKEN`. No extra release token or package-update commit is required.
The npm job runs in the `Release` environment, so a reviewer approves it in the workflow run before it publishes.

The Homebrew formula lives in `Homebrew/homebrew-core` and builds from the release tag's source archive.
Homebrew's autobump automation detects the new tag and updates the formula, so the release needs no Homebrew step.
Homebrew availability can lag behind npm and GitHub publication.
There is no official Scoop package; do not advertise Scoop installation until `ScoopInstaller/Main` accepts one.
