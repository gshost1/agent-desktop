#!/usr/bin/env python3
"""Free disk by removing bench cargo target dirs whose source worktree belongs to a merged or
closed fork PR. Only ~/.agent-desktop-bench-target/trees/<name> dirs are ever removed.

A target is removed only when its `.source-tree` worktree still exists, is not on main, its HEAD
is not on origin/main's first-parent line, and every fork PR whose head branch or head commit
matches it is MERGED or CLOSED (at least one must match). A target with no matching PR, an open
PR, or an unreadable source is kept. Run it under the build lock (serial_cargo.sh) so no build
is using a target while it goes.

  python3 bench/prune_merged_targets.py [--min-free-gb 30] [--dry-run]
"""
import json
import os
import shutil
import subprocess
import sys

TREES = os.path.expanduser("~/.agent-desktop-bench-target/trees")
REPO = "gshost1/agent-desktop"


def free_gb():
    return shutil.disk_usage(os.path.expanduser("~")).free / 1e9


def git(source, *args):
    done = subprocess.run(["git", "-C", source, *args], capture_output=True, text=True, timeout=60)
    return done.stdout.strip() if done.returncode == 0 else ""


def size_gb(path):
    out = subprocess.run(["du", "-sk", path], capture_output=True, text=True).stdout.split()
    return int(out[0]) / 1e6 if out else 0.0


def fork_prs():
    out = subprocess.run(["gh", "pr", "list", "--repo", REPO, "--state", "all", "--limit", "400",
                          "--json", "number,state,headRefName,headRefOid"],
                         capture_output=True, text=True, timeout=120)
    if out.returncode != 0:
        return None
    return json.loads(out.stdout)


def verdict(source, prs, main_line):
    if not source or not os.path.isdir(source):
        return None, "source missing"
    head = git(source, "rev-parse", "HEAD")
    branch = git(source, "symbolic-ref", "--quiet", "--short", "HEAD")
    if not head:
        return None, "unreadable HEAD"
    if branch in ("main", "master") or head in main_line:
        return None, "main"
    matches = [p for p in prs if (branch and p["headRefName"] == branch) or p["headRefOid"] == head]
    if not matches:
        return None, "no PR"
    if any(p["state"] == "OPEN" for p in matches):
        return None, "open PR #" + ",".join(str(p["number"]) for p in matches if p["state"] == "OPEN")
    return matches, "closed or merged PR #" + ",".join(str(p["number"]) for p in matches)


def main():
    args = sys.argv[1:]
    dry = "--dry-run" in args
    floor = float(args[args.index("--min-free-gb") + 1]) if "--min-free-gb" in args else 30.0
    before = free_gb()
    print(f"prune: free {before:.1f} GB, floor {floor:.0f} GB{' (dry run)' if dry else ''}")
    if before >= floor and not dry:
        print("prune: above the floor; nothing removed")
        return 0
    prs = fork_prs()
    if prs is None:
        print("prune: could not list fork PRs; nothing removed")
        return 0
    here = os.path.dirname(os.path.abspath(__file__))
    main_line = set(git(here, "rev-list", "--first-parent", "-n", "2000", "origin/main").split())
    removed = 0.0
    for name in sorted(os.listdir(TREES)):
        target = os.path.join(TREES, name)
        if not os.path.isdir(target) or os.path.islink(target):
            continue
        try:
            with open(os.path.join(target, ".source-tree")) as fh:
                source = fh.readline().strip()
        except OSError:
            source = ""
        matches, why = verdict(source, prs, main_line)
        if not matches:
            print(f"prune: keep {name} ({why})")
            continue
        gb = size_gb(target)
        print(f"prune: {'would remove' if dry else 'remove'} {name} {gb:.1f} GB ({why}, source {source})")
        if not dry:
            shutil.rmtree(target, ignore_errors=True)
            removed += gb
    print(f"prune: removed {removed:.1f} GB; free now {free_gb():.1f} GB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
