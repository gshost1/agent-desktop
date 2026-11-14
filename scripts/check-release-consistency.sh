#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

lock_version_for() {
  local package="$1"
  awk -v target="$package" '
    /^\[\[package\]\]/ {
      if (name == target) {
        print version
        found = 1
        exit
      }
      name = ""
      version = ""
      next
    }
    /^name = / {
      name = $3
      gsub(/"/, "", name)
      next
    }
    /^version = / {
      version = $3
      gsub(/"/, "", version)
      next
    }
    END {
      if (!found && name == target) {
        print version
      }
    }
  ' Cargo.lock
}

workspace_version="$(node scripts/release-version.js)"

packages=(
  agent-desktop
  agent-desktop-core
  agent-desktop-ffi
  agent-desktop-linux
  agent-desktop-macos
  agent-desktop-windows
)

for package in "${packages[@]}"; do
  lock_version="$(lock_version_for "$package")"
  [[ -n "$lock_version" ]] || fail "Cargo.lock is missing ${package}"
  [[ "$lock_version" == "$workspace_version" ]] \
    || fail "Cargo.lock ${package} version ${lock_version} does not match Cargo.toml ${workspace_version}"
done

changelog_version="$(sed -n 's/^## \[\([^]]*\)\].*/\1/p' CHANGELOG.md | head -n 1)"
[[ "$changelog_version" == "$workspace_version" ]] \
  || fail "CHANGELOG.md tops out at ${changelog_version:-no release}, Cargo.toml is ${workspace_version}; run python3 scripts/changelog_fragments.py assemble --version ${workspace_version} in the release pull request"

PYTHONDONTWRITEBYTECODE=1 python3 scripts/changelog_fragments_test.py >/dev/null 2>&1 \
  || fail "the changelog fragment self-test fails; rerun python3 scripts/changelog_fragments_test.py for detail"
PYTHONDONTWRITEBYTECODE=1 python3 scripts/changelog_fragments.py check >/dev/null \
  || fail "a fragment in changelog.d/ is malformed; rerun python3 scripts/changelog_fragments.py check for detail"

cargo metadata --locked --no-deps --format-version 1 >/dev/null
echo "OK: release metadata is internally consistent for ${workspace_version}"
