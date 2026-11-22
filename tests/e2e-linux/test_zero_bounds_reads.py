import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from scenarios.zero_bounds import zero_extents


class ZeroBoundsReadTests(unittest.TestCase):
    def test_each_node_is_read_once_per_observation(self):
        bounds = [(1, 2, 0, 10), (1, 2, 10, 0), (1, 2, 10, 10)]
        nodes = [SimpleNamespace(path=str(i), role=39, extents=Mock(return_value=rect))
                 for i, rect in enumerate(bounds)]
        self.assertEqual(zero_extents(nodes), [("0", 39, bounds[0]), ("1", 39, bounds[1])])
        for node in nodes:
            node.extents.assert_called_once_with()

    def test_empty_observation_has_no_zero_containers(self):
        self.assertEqual(zero_extents([]), [])

    def test_a_later_observation_does_not_reuse_old_geometry(self):
        node = SimpleNamespace(path="/node", role=39, extents=Mock(side_effect=[(0, 0, 0, 1), (0, 0, 1, 1)]))
        self.assertEqual(zero_extents([node]), [("/node", 39, (0, 0, 0, 1))])
        self.assertEqual(zero_extents([node]), [])
        self.assertEqual(node.extents.call_count, 2)

    def test_geometry_errors_are_not_hidden(self):
        node = SimpleNamespace(path="/node", role=39, extents=Mock(side_effect=RuntimeError("unavailable")))
        with self.assertRaisesRegex(RuntimeError, "unavailable"):
            zero_extents([node])
