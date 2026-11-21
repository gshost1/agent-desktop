"""Decide whether a CI event needs the macOS runner jobs.

GitHub Free shares about five concurrent macOS jobs across an account, so a
pull request runs them only when it can change what macOS builds or tests.
A push, a manual dispatch and any unreadable diff run them. A draft pull
request never does; marking it ready for review starts them.

Prints `macos=true` or `macos=false` for `$GITHUB_OUTPUT`.
"""

import argparse
import subprocess
import sys

NON_MACOS_PREFIXES = (
    "crates/linux/",
    "crates/windows/",
    "tests/e2e-linux/",
    "tests/e2e-windows/",
    "probes/linux/",
    "probes/windows/",
    "skills/agent-desktop-linux/",
    "skills/agent-desktop-windows/",
    "apps/docs/",
    "docs/",
    "changelog.d/",
)

NON_MACOS_FILES = frozenset(
    {
        ".github/workflows/linux-e2e.yml",
        ".github/workflows/windows-e2e.yml",
        ".github/workflows/windows-capability-probe.yml",
        ".github/workflows/windows-competitive-latency.yml",
        ".github/workflows/codeql.yml",
        ".github/workflows/supply-chain.yml",
        "scripts/check-linux-capture-cli.py",
        "scripts/check-linux-findings.py",
        "scripts/ffi-archive-readme-linux.md",
        "scripts/linux-live-gate.py",
        "scripts/linux_live_gate_rules.py",
        "scripts/test-linux-live-gate.py",
    }
)


def checked_by_macos_bash(path):
    """The macOS job parses these with /bin/bash 3.2 (scripts/check-bash3-compat.sh)."""
    if path == ".githooks/pre-commit":
        return True
    for folder in ("scripts/", "tests/e2e/", "tests/e2e-linux/"):
        name = path[len(folder):] if path.startswith(folder) else ""
        if name.endswith(".sh") and "/" not in name:
            return True
    return False


def path_needs_macos(path):
    if checked_by_macos_bash(path):
        return True
    if path in NON_MACOS_FILES or path.startswith(NON_MACOS_PREFIXES):
        return False
    if path.startswith("scripts/") and path.endswith(".ps1"):
        return False
    if path.endswith(".md") and not path.startswith("skills/"):
        return False
    return True


def changes_need_macos(paths):
    paths = [path for path in paths if path]
    if not paths:
        return True
    return any(path_needs_macos(path) for path in paths)


def event_needs_macos(event, draft, changed_paths):
    if event != "pull_request":
        return True
    if draft == "true":
        return False
    if changed_paths is None:
        return True
    return changes_need_macos(changed_paths)


def read_merge_commit_changes():
    result = subprocess.run(
        ["git", "diff", "--no-renames", "--name-only", "HEAD^1", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        return None
    return result.stdout.splitlines()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True)
    parser.add_argument("--draft", default="false")
    args = parser.parse_args(argv)
    changed = None
    if args.event == "pull_request" and args.draft != "true":
        changed = read_merge_commit_changes()
    decision = event_needs_macos(args.event, args.draft, changed)
    print(f"macos={'true' if decision else 'false'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
