import sys
import unittest
from pathlib import Path
from unittest.mock import Mock, call

sys.path.insert(0, str(Path(__file__).resolve().parent))

from oracle.atspi_raw import RawNode


class GeometryTests(unittest.TestCase):
    def test_position_and_size_do_not_use_window_extents_bottom_right(self):
        for window_coords in (False, True):
            with self.subTest(window_coords=window_coords):
                node = RawNode(None, "bus", "/node")
                node.invoke = Mock(side_effect=[(9, 47), (502, 22)])
                self.assertEqual(node.extents(window_coords), (9, 47, 502, 22))
                self.assertEqual(node.invoke.call_args_list, [
                    call("org.a11y.atspi.Component", "GetPosition", "(u)",
                         (int(window_coords),)),
                    call("org.a11y.atspi.Component", "GetSize")])

    def test_zero_bounds_remain_zero(self):
        node = RawNode(None, "bus", "/node")
        node.invoke = Mock(side_effect=[(-1, -1), (0, 0)])
        self.assertEqual(node.extents(), (-1, -1, 0, 0))

    def test_position_or_size_errors_are_not_hidden(self):
        for results in ([RuntimeError("position unavailable")],
                        [(9, 47), RuntimeError("size unavailable")]):
            with self.subTest(results=results):
                node = RawNode(None, "bus", "/node")
                node.invoke = Mock(side_effect=results)
                with self.assertRaisesRegex(RuntimeError, "unavailable"):
                    node.extents()
