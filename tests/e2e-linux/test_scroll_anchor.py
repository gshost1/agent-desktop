import copy
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from runner.legs import REGISTRY
from scenarios.ref_scroll import identity_refusal, inside, no_input, scroll_ref


class ScrollAnchorTests(unittest.TestCase):
    def test_visibility_requires_full_vertical_containment_without_edge_tolerance(self):
        node = SimpleNamespace(extents=Mock(side_effect=[(4, 104, 360, 40), (4, -36, 360, 40),
                                                       (4, 144, 360, 40)]))
        self.assertTrue(inside(node, (0, 0, 360, 180)))
        self.assertFalse(inside(node, (0, 0, 360, 180)))
        self.assertFalse(inside(node, (0, 0, 360, 180)))

    def test_selects_the_only_scroll_area_even_without_a_name(self):
        tree = {"children": [{"ref_id": "@s:e1", "role": "scrollarea"},
                             {"ref_id": "@s:e2", "role": "checkbox", "name": "Upper marker"}]}
        self.assertEqual(scroll_ref({"tree": tree}), "@s:e1")

    def test_empty_missing_ref_wrong_role_and_multiple_scroll_areas_fail(self):
        target = {"ref_id": "@s:e1", "role": "scrollarea"}
        for tree in (None, {}, {"role": "scrollarea"}, {**target, "role": "checkbox"},
                     {"children": [target, {**target, "ref_id": "@s:e2"}]},
                     {"children": [target, {"role": "scrollarea"}]}):
            with self.subTest(tree=tree), self.assertRaisesRegex(AssertionError, "unique scroll"):
                scroll_ref({"tree": tree})

    def test_settings_requires_exact_identity_refusal_with_no_input(self):
        error = {"code": "AMBIGUOUS_TARGET", "details": {"kind": "identity_not_unique"},
                 "disposition": {"delivery": "not_delivered", "retry": "safe"}}
        response = SimpleNamespace(ok=False, error=error, raw="refusal")
        identity_refusal(response)
        for key, value in (("code", "POLICY_DENIED"), ("details", {"kind": "identity_unproven"}),
                           ("disposition", {"delivery": "delivered_unverified", "retry": "unsafe"}),
                           ("details", {"kind": "identity_not_unique", "delivered_clicks": 1}),
                           ("details", {"kind": "identity_not_unique", "physical_delivery_started": True})):
            changed = copy.deepcopy(error)
            changed[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(AssertionError):
                identity_refusal(SimpleNamespace(ok=False, error=changed, raw="bad refusal"))
        for response in (SimpleNamespace(ok=True, error=error, raw="success"),
                         SimpleNamespace(ok=False, error=None, raw="missing error")):
            with self.assertRaises(AssertionError):
                identity_refusal(response)

    def test_headless_policy_refusal_cannot_claim_input(self):
        with self.assertRaises(AssertionError):
            no_input(SimpleNamespace(error={"disposition": {"delivery": "not_delivered", "retry": "unsafe"}}, raw="bad"))

    def test_settings_leg_is_not_positive_scroll_coverage(self):
        leg = next(leg for leg in REGISTRY if leg.id == "ref_actions.settings_scroll_identity_refusal")
        self.assertEqual(leg.modes, ("headless", "headed"))
        self.assertEqual(leg.disposition, "asserted-refusal")
