from __future__ import annotations

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from oracle import atspi_raw as raw
from scenarios import surfaces

GONE = ("g-dbus-error-quark: GDBus.Error:org.freedesktop.DBus.Error.UnknownMethod: "
        "Object does not exist at path “/org/gnome/Calculator/a11y/97fc” (19)")


class Item:
    states = frozenset({"SHOWING"})


class Root:
    def __init__(self, answers):
        self.answers = list(answers)

    def find(self, *, role=None, name=None):
        answer = self.answers.pop(0)
        if isinstance(answer, Exception):
            raise answer
        return answer


class SurfaceSettlingTests(unittest.TestCase):
    def test_a_vanished_object_reads_as_vanished_and_other_errors_do_not(self):
        self.assertTrue(raw.vanished(RuntimeError(GONE)))
        self.assertTrue(raw.vanished(RuntimeError("org.freedesktop.DBus.Error.UnknownObject: x")))
        self.assertFalse(raw.vanished(RuntimeError("org.freedesktop.DBus.Error.UnknownMethod: no Foo")))
        self.assertFalse(raw.vanished(TimeoutError("oracle responsiveness deadline expired")))

    def test_unless_vanished_returns_none_only_for_a_vanished_object(self):
        def gone():
            raise RuntimeError(GONE)

        def broken():
            raise RuntimeError("org.freedesktop.DBus.Error.AccessDenied")

        self.assertIsNone(surfaces.unless_vanished(gone))
        self.assertEqual(surfaces.unless_vanished(lambda: 3), 3)
        with self.assertRaises(RuntimeError):
            surfaces.unless_vanished(broken)

    def test_menu_items_settle_after_a_node_vanishes_mid_walk(self):
        items = [Item(), Item()]
        tree = Root([[Item()], RuntimeError(GONE), items, items])
        app = SimpleNamespace(owner_names=["Calculator"])
        ctx = SimpleNamespace(tier="x11-openbox")
        with patch.object(surfaces, "_toggle"), \
                patch.object(surfaces, "root", return_value=tree), \
                patch.object(surfaces.raw.time, "sleep"):
            self.assertEqual(surfaces.about_items(ctx, app), items)
        self.assertEqual(tree.answers, [])


if __name__ == "__main__":
    unittest.main()
