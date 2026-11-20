import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from scenarios import observation


class MenuLabelTests(unittest.TestCase):
    def node(self, name="", states=("SHOWING",), relations=()):
        return SimpleNamespace(name=name, states=set(states), connection="connection", bus="owner",
                               invoke=Mock(return_value=(relations,)))

    def showing(self, nodes):
        tree = Mock()
        tree.find.return_value = nodes
        result = observation.preferences_showing(tree)
        tree.find.assert_called_once_with(role=35)
        return result

    def test_visible_named_item_does_not_need_label_relations(self):
        node = self.node("Preferences")
        self.assertTrue(self.showing([node]))
        node.invoke.assert_not_called()

    def test_labelled_by_text_is_read_independently_of_the_product(self):
        node = self.node(relations=[(2, [("", "/label")])])
        label = Mock()
        label.name = ""
        label.text.return_value = "Preferences"
        with patch.object(observation.raw, "RawNode", return_value=label) as raw_node:
            self.assertTrue(self.showing([node]))
        raw_node.assert_called_once_with("connection", "owner", "/label")
        node.invoke.assert_called_once_with(observation.raw.ACCESSIBLE, "GetRelationSet")

    def test_empty_hidden_unlabelled_and_unrelated_items_do_not_resolve(self):
        for nodes in ([], [self.node("Preferences", states=())], [self.node()],
                      [self.node("Other")], [self.node(relations=[(1, [("bus", "/other")])])]):
            with self.subTest(nodes=nodes):
                self.assertFalse(self.showing(nodes))

    def test_ambiguous_labels_and_read_failures_are_not_accepted(self):
        node = self.node(relations=[(2, [("bus", "/one"), ("bus", "/two")])])
        with self.assertRaisesRegex(AssertionError, "ambiguous labels"):
            self.showing([node])
        node = self.node()
        node.invoke.side_effect = RuntimeError("native read failed")
        with self.assertRaisesRegex(RuntimeError, "native read failed"):
            self.showing([node])
