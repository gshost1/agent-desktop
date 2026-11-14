"""Run with python3 scripts/changelog_fragments_test.py."""

import contextlib
import importlib.util
import io
import shutil
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
FIXTURES = SCRIPTS / "fixtures" / "changelog-fragments"
SPEC = importlib.util.spec_from_file_location(
    "changelog_fragments", SCRIPTS / "changelog_fragments.py"
)
FRAGMENTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FRAGMENTS)


def run(*argv):
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = FRAGMENTS.main(list(argv))
    return code, out.getvalue(), err.getvalue()


class ReleaseTree:
    def __enter__(self):
        self.directory = tempfile.mkdtemp(prefix="changelog-fragments-")
        self.root = Path(self.directory) / "repo"
        shutil.copytree(FIXTURES / "release", self.root)
        (self.root / "expected-CHANGELOG.md").unlink()
        return self.root

    def __exit__(self, *exc):
        shutil.rmtree(self.directory)


ASSEMBLE = ("assemble", "--version", "1.5.0", "--date", "2026-11-20")


class CheckTests(unittest.TestCase):
    def test_every_invalid_fixture_fails(self):
        invalid = sorted((FIXTURES / "invalid").iterdir())
        self.assertGreaterEqual(len(invalid), 12)
        for path in invalid:
            with self.subTest(fixture=path.name):
                code, _, err = run("check", str(path))
                self.assertEqual(code, 1)
                self.assertIn(path.name, err)

    def test_valid_fixtures_pass(self):
        for path in sorted((FIXTURES / "release" / "changelog.d").glob("*.md")):
            with self.subTest(fixture=path.name):
                self.assertEqual(run("check", str(path))[0], 0)

    def test_missing_fragment_directory_fails(self):
        with ReleaseTree() as root:
            shutil.rmtree(root / "changelog.d")
            self.assertEqual(run("--root", str(root), "check")[0], 1)

    def test_reads_the_top_release_heading(self):
        text = (FIXTURES / "release" / "CHANGELOG.md").read_text(encoding="utf-8")
        self.assertEqual(
            FRAGMENTS.top_section(text),
            ("1.4.0", "https://github.com/gshost1/agent-desktop"),
        )


class AssembleTests(unittest.TestCase):
    def test_assembles_in_heading_order_and_keeps_bytes(self):
        with ReleaseTree() as root:
            code, _, err = run("--root", str(root), *ASSEMBLE)
            self.assertEqual(code, 0, err)
            expected = (FIXTURES / "release" / "expected-CHANGELOG.md").read_bytes()
            self.assertEqual((root / "CHANGELOG.md").read_bytes(), expected)
            remaining = sorted(p.name for p in (root / "changelog.d").iterdir())
            self.assertEqual(remaining, [".gitkeep"])

    def test_every_fragment_bullet_lands_verbatim(self):
        with ReleaseTree() as root:
            bullets = []
            for path in (root / "changelog.d").glob("*.md"):
                bullets += [
                    line
                    for line in path.read_bytes().split(b"\n")
                    if line.startswith(b"* ")
                ]
            run("--root", str(root), *ASSEMBLE)
            lines = (root / "CHANGELOG.md").read_bytes().split(b"\n")
            for bullet in bullets:
                self.assertEqual(lines.count(bullet), 1, bullet)

    def test_intro_follows_the_release_heading(self):
        with ReleaseTree() as root:
            run("--root", str(root), *ASSEMBLE, "--intro", "Fork release.")
            lines = (root / "CHANGELOG.md").read_text(encoding="utf-8").split("\n")
            self.assertTrue(lines[2].startswith("## [1.5.0]"))
            self.assertEqual(lines[3:6], ["", "Fork release.", ""])

    def test_dry_run_changes_nothing(self):
        with ReleaseTree() as root:
            before = (root / "CHANGELOG.md").read_bytes()
            code, out, _ = run("--root", str(root), *ASSEMBLE, "--dry-run")
            self.assertEqual(code, 0)
            self.assertTrue(out.startswith("## [1.5.0]"))
            self.assertEqual((root / "CHANGELOG.md").read_bytes(), before)
            self.assertEqual(len(list((root / "changelog.d").glob("*.md"))), 2)

    def test_refuses_and_keeps_everything_on_a_bad_fragment(self):
        with ReleaseTree() as root:
            shutil.copy(FIXTURES / "invalid" / "unknown-heading.md", root / "changelog.d")
            before = (root / "CHANGELOG.md").read_bytes()
            self.assertEqual(run("--root", str(root), *ASSEMBLE)[0], 1)
            self.assertEqual((root / "CHANGELOG.md").read_bytes(), before)
            self.assertEqual(len(list((root / "changelog.d").glob("*.md"))), 3)

    def test_refuses_a_released_version(self):
        with ReleaseTree() as root:
            code, _, err = run("--root", str(root), "assemble", "--version", "1.4.0")
            self.assertEqual(code, 1)
            self.assertIn("already has", err)

    def test_refuses_with_no_fragment(self):
        with ReleaseTree() as root:
            for path in (root / "changelog.d").glob("*.md"):
                path.unlink()
            code, _, err = run("--root", str(root), *ASSEMBLE)
            self.assertEqual(code, 1)
            self.assertIn("no fragment", err)

    def test_refuses_bad_version_and_date(self):
        with ReleaseTree() as root:
            self.assertEqual(run("--root", str(root), "assemble", "--version", "v1.5")[0], 1)
            self.assertEqual(
                run("--root", str(root), "assemble", "--version", "1.5.0", "--date", "soon")[0],
                1,
            )


if __name__ == "__main__":
    unittest.main()
