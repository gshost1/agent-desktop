#!/usr/bin/env python3
"""Check and assemble the per-PR changelog fragments in changelog.d/.

Every pull request adds one fragment, changelog.d/<branch with / replaced by ->.md,
and never edits CHANGELOG.md. A fragment holds `### <Heading>` sections, each
followed by a blank line and one or more single-line `* ` bullets. The release
pull request runs `assemble`, which writes one new `## [x.y.z](compare link) (date)`
section at the top of CHANGELOG.md, keeps every bullet byte for byte, and deletes
the fragments it consumed.

    python3 scripts/changelog_fragments.py check [FRAGMENT ...]
    python3 scripts/changelog_fragments.py assemble --version 1.5.0 [--date 2026-11-20]
        [--previous 1.4.0] [--intro "One paragraph."] [--dry-run]

`check` with no paths checks every fragment in changelog.d/. Standard library only.
"""

import argparse
import datetime
import re
import sys
from pathlib import Path

HEADINGS = (
    "BREAKING CHANGES",
    "Features",
    "Bug Fixes",
    "Performance",
    "Documentation",
    "Testing",
    "Known issues",
)
FRAGMENT_DIR = "changelog.d"
KEEP_FILE = ".gitkeep"
NAME_PATTERN = re.compile(r"^[a-z0-9][a-z0-9._-]*\.md$")
VERSION_PATTERN = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")
DATE_PATTERN = re.compile(r"^\d{4}-\d{2}-\d{2}$")
SECTION_PATTERN = re.compile(
    r"^## \[(?P<version>[^\]]+)\]\((?P<base>https://[^)]+)/compare/v[^)]+\)"
)
PULL_PATTERN = re.compile(r"/pull/(\d+)\)")


class FragmentError(Exception):
    pass


def parse_fragment(path):
    """Return [(heading, [bullet line, ...]), ...] or raise FragmentError."""
    problems = []
    if not NAME_PATTERN.match(path.name):
        problems.append(
            "file name must be the branch name with / replaced by -, "
            "lowercase, ending in .md"
        )
    raw = path.read_bytes()
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        raise FragmentError(f"{path}: not UTF-8")
    if "\r" in text:
        problems.append("uses CR line endings; use LF only")
    if not text.endswith("\n"):
        problems.append("must end with a newline")
    sections = []
    seen = set()
    previous_blank = True
    for number, line in enumerate(text.split("\n")[:-1], start=1):
        where = f"line {number}"
        if line == "":
            previous_blank = True
            continue
        if line != line.rstrip():
            problems.append(f"{where}: trailing whitespace")
        if line.startswith("### "):
            heading = line[4:]
            if heading not in HEADINGS:
                problems.append(
                    f"{where}: unknown heading {heading!r}; use one of "
                    + ", ".join(HEADINGS)
                )
            elif heading in seen:
                problems.append(f"{where}: heading {heading!r} appears twice")
            if not previous_blank:
                problems.append(f"{where}: a heading needs a blank line before it")
            seen.add(heading)
            sections.append((heading, []))
        elif line.startswith("* ") and line[2:].strip():
            if not sections:
                problems.append(f"{where}: a bullet before any ### heading")
            elif not sections[-1][1] and not previous_blank:
                problems.append(f"{where}: put a blank line after the heading")
            else:
                sections[-1][1].append(line)
        else:
            problems.append(
                f"{where}: only '### <Heading>' lines, blank lines and single-line "
                "'* ' bullets are allowed"
            )
        previous_blank = False
    if not sections and not problems:
        problems.append("holds no ### section")
    for heading, bullets in sections:
        if not bullets:
            problems.append(f"section {heading!r} has no bullet")
    if problems:
        raise FragmentError("\n".join(f"{path}: {problem}" for problem in problems))
    return sections


def fragment_paths(root):
    directory = root / FRAGMENT_DIR
    if not directory.is_dir():
        raise FragmentError(f"{directory} is missing; it must exist with a {KEEP_FILE}")
    paths = []
    for path in sorted(directory.iterdir()):
        if path.name == KEEP_FILE:
            continue
        if path.is_dir() or path.suffix != ".md":
            raise FragmentError(f"{path}: only .md fragments belong in {FRAGMENT_DIR}/")
        paths.append(path)
    return paths


def check(paths):
    failures = []
    for path in paths:
        try:
            parse_fragment(path)
        except FragmentError as error:
            failures.append(str(error))
    return failures


def bullet_order(bullet, fragment_index):
    pulls = PULL_PATTERN.findall(bullet)
    if pulls:
        return (0, int(pulls[0]), fragment_index)
    return (1, 0, fragment_index)


def build_section(fragments, header, intro):
    grouped = {heading: [] for heading in HEADINGS}
    for index, sections in enumerate(fragments):
        for heading, bullets in sections:
            for bullet in bullets:
                grouped[heading].append((bullet_order(bullet, index), bullet))
    lines = [header, ""]
    if intro:
        lines += [intro, ""]
    for heading in HEADINGS:
        entries = sorted(grouped[heading], key=lambda entry: entry[0])
        if not entries:
            continue
        lines += [f"### {heading}", ""]
        lines += [bullet for _, bullet in entries]
        lines.append("")
    return "\n".join(lines) + "\n"


def top_section(changelog_text):
    for line in changelog_text.split("\n"):
        match = SECTION_PATTERN.match(line)
        if match:
            return match.group("version"), match.group("base")
        if line.startswith("## "):
            raise FragmentError(f"CHANGELOG.md: cannot read the release heading {line!r}")
    raise FragmentError("CHANGELOG.md: no '## [x.y.z](compare link) (date)' section")


def insert_section(changelog_text, section):
    marker = "\n## "
    index = changelog_text.find(marker)
    if index < 0:
        raise FragmentError("CHANGELOG.md: no release section to insert above")
    return changelog_text[: index + 1] + section + changelog_text[index + 1 :]


def assemble(root, options):
    if not VERSION_PATTERN.match(options.version):
        raise FragmentError(f"--version {options.version!r} is not x.y.z")
    changelog = root / "CHANGELOG.md"
    text = changelog.read_bytes().decode("utf-8")
    top_version, base = top_section(text)
    if re.search(r"^## \[" + re.escape(options.version) + r"\]", text, re.MULTILINE):
        raise FragmentError(f"CHANGELOG.md already has a {options.version} section")
    previous = options.previous or top_version
    if not VERSION_PATTERN.match(previous):
        raise FragmentError(f"previous version {previous!r} is not x.y.z")
    date = options.date or datetime.date.today().isoformat()
    if not DATE_PATTERN.match(date):
        raise FragmentError(f"--date {date!r} is not YYYY-MM-DD")
    paths = fragment_paths(root)
    if not paths:
        raise FragmentError(f"{FRAGMENT_DIR}/ holds no fragment to release")
    failures = check(paths)
    if failures:
        raise FragmentError("\n".join(failures))
    fragments = [parse_fragment(path) for path in paths]
    header = f"## [{options.version}]({base}/compare/v{previous}...v{options.version}) ({date})"
    section = build_section(fragments, header, options.intro)
    if options.dry_run:
        sys.stdout.write(section)
        return
    changelog.write_bytes(insert_section(text, section).encode("utf-8"))
    for path in paths:
        path.unlink()
    print(f"CHANGELOG.md: wrote {options.version} from {len(paths)} fragment(s) and deleted them")


def parse_arguments(argv):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--root", default=str(Path(__file__).resolve().parent.parent))
    commands = parser.add_subparsers(dest="command", required=True)
    check_parser = commands.add_parser("check")
    check_parser.add_argument("paths", nargs="*")
    assemble_parser = commands.add_parser("assemble")
    assemble_parser.add_argument("--version", required=True)
    assemble_parser.add_argument("--previous")
    assemble_parser.add_argument("--date")
    assemble_parser.add_argument("--intro")
    assemble_parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args(argv)


def main(argv):
    options = parse_arguments(argv)
    root = Path(options.root)
    try:
        if options.command == "check":
            paths = [Path(p) for p in options.paths] or fragment_paths(root)
            failures = check(paths)
            if failures:
                print("\n".join(failures), file=sys.stderr)
                return 1
            print(f"OK: {len(paths)} changelog fragment(s) are well formed")
            return 0
        assemble(root, options)
        return 0
    except FragmentError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
