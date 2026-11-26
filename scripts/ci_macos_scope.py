"""Decide whether a CI event starts the GitHub-hosted macOS runner jobs.

macOS is tested on the self-hosted ARM macOS queue. GitHub Free shares about
five concurrent hosted macOS jobs across an account, and hosted jobs queued
from several pull requests at once were cancelled before they started. So a
pull request and a push never start them; a manual dispatch does.

Prints `macos=true` or `macos=false` for `$GITHUB_OUTPUT`.
"""

import argparse
import sys


def event_needs_macos(event):
    return event == "workflow_dispatch"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event", required=True)
    parser.add_argument("--draft", default="false")
    args = parser.parse_args(argv)
    print(f"macos={'true' if event_needs_macos(args.event) else 'false'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
