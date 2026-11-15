import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))

from runner import helper
from runner.product import Envelope
from scenarios import launch, select, surfaces


def envelope(ok, data=None, error=None):
    return Envelope(ok, data, error, "{}", 0, [])


HANDOFF = {"code": "ACTION_FAILED", "details": {"kind": "exited", "status": 0, "spawned_pid": 9},
           "disposition": {"delivery": "delivered_unverified", "retry": "unsafe"}}


class TerminalHandoffTests(unittest.TestCase):
    def test_only_the_documented_clean_exit_handoff_is_accepted(self):
        self.assertTrue(launch.unresolved_handoff(envelope(False, error=HANDOFF)))
        for change in ({"code": "TIMEOUT"}, {"details": {"kind": "exited", "status": 1}},
                       {"details": {"kind": "spawn_failed", "status": 0}},
                       {"disposition": {"delivery": "not_delivered"}}):
            with self.subTest(change=change):
                self.assertFalse(launch.unresolved_handoff(envelope(False, error={**HANDOFF, **change})))
        self.assertFalse(launch.unresolved_handoff(envelope(True, data={})))


class HeadlessSelectRefusalTests(unittest.TestCase):
    def test_only_an_unsent_pointer_required_refusal_passes(self):
        refusal = {"code": "POLICY_DENIED", "details": {"kind": "pointer_required"},
                   "disposition": {"delivery": "not_delivered"}}
        select.refused_headless(None, envelope(False, error=refusal))
        for change in ({"code": "ACTION_NOT_SUPPORTED"}, {"details": {"kind": "supported_action"}},
                       {"disposition": {"delivery": "delivered_unverified"}}):
            with self.subTest(change=change), self.assertRaises(AssertionError):
                select.refused_headless(None, envelope(False, error={**refusal, **change}))


class ListedNameTests(unittest.TestCase):
    def ctx(self, apps):
        return SimpleNamespace(product=lambda argv: envelope(True, data={"apps": apps}))

    def test_the_name_list_apps_gives_the_owned_pid_is_used(self):
        app = SimpleNamespace(pid=42)
        ctx = self.ctx([{"name": "python3", "pid": 42}, {"name": "other", "pid": 7}])
        self.assertEqual(helper.listed_name(ctx, app), "python3")

    def test_a_pid_listed_never_or_twice_is_refused(self):
        app = SimpleNamespace(pid=42)
        for apps in ([], [{"name": "a", "pid": 42}, {"name": "b", "pid": 42}]):
            with self.subTest(apps=apps), self.assertRaises(AssertionError):
                helper.listed_name(self.ctx(apps), app)


class MenuSettleTests(unittest.TestCase):
    def test_a_node_that_vanished_mid_walk_is_read_again_and_other_errors_stay(self):
        from unittest.mock import patch
        item = SimpleNamespace(states={"SHOWING"})
        gone = RuntimeError("GDBus.Error:org.freedesktop.DBus.Error.UnknownMethod: "
                            "Object does not exist at path /org/app/a11y/1")
        reads = iter([gone, [item], [item]])

        def find(role):
            value = next(reads)
            if isinstance(value, Exception):
                raise value
            return value

        tree = SimpleNamespace(find=find)
        app = SimpleNamespace(owner_names=[":1.2"])
        with patch.object(surfaces, "_toggle"), patch.object(surfaces, "root", return_value=tree), \
                patch.object(surfaces.raw, "poll", side_effect=lambda check, *_a, **_k:
                             next(value for value in iter(check, object()) if value)):
            self.assertEqual(surfaces.about_items(SimpleNamespace(tier=None), app), [item])
        other = SimpleNamespace(find=lambda role: (_ for _ in ()).throw(RuntimeError("NoReply")))
        with patch.object(surfaces, "_toggle"), patch.object(surfaces, "root", return_value=other), \
                patch.object(surfaces.raw, "poll", side_effect=lambda check, *_a, **_k: check()):
            with self.assertRaisesRegex(RuntimeError, "NoReply"):
                surfaces.about_items(SimpleNamespace(tier=None), app)


if __name__ == "__main__":
    unittest.main()
