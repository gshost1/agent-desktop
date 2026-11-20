import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from runner import apps, gate, main, report, verdict


class MouseFixtureGateTests(unittest.TestCase):
    def test_umockdev_is_not_a_tier_prerequisite(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "usr/bin").mkdir(parents=True)
            (root / "usr/bin/openbox").touch()
            binary = root / "agent-desktop"
            binary.touch()
            env = {"AGENT_DESKTOP_WM_ROOT": directory, "AGENT_DESKTOP_E2E_BIN": str(binary),
                   "AGENT_DESKTOP_SESSION_BASE": directory}
            self.assertEqual(gate.check_prerequisites(("x11-openbox",), env=env,
                which=lambda name: None if name == "umockdev-run" else "/bin/" + name,
                platform="linux", gi_probe=lambda: None), [])

    def test_missing_fixture_marks_only_that_leg_unavailable(self):
        context = Mock()
        context.findings = []
        context.owned.return_value.members = frozenset()
        context.baseline.members = frozenset()
        leg = SimpleNamespace(id="fixture")
        def unavailable(ctx):
            raise apps.FixtureUnavailable("missing umockdev-run")
        with patch.object(verdict, "LegContext", return_value=context), \
                patch.object(main, "_write_evidence"):
            result = main._run_leg(unavailable, leg, None, "x11-openbox", "headless", Path("/unused"),
                                   SimpleNamespace(evaluate=lambda ctx: []), report)
            healthy = main._run_leg(lambda ctx: None, leg, None, "x11-openbox", "headless", Path("/unused"),
                                    SimpleNamespace(evaluate=lambda ctx: []), report)
        self.assertEqual(result.outcome, report.TIER_UNAVAILABLE)
        self.assertIn("umockdev-run", result.detail)
        self.assertEqual(healthy.outcome, report.PASS)
