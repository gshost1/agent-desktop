import sys
import unittest
from pathlib import Path
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

from apps.zoo_qt import toolbar_overflow


class ToolbarOverflowTests(unittest.TestCase):
    def test_finds_the_toolbar_overflow_button(self):
        button = object()
        button_type = object()
        toolbar = Mock()
        toolbar.findChild.return_value = button
        self.assertIs(toolbar_overflow(toolbar, button_type), button)
        toolbar.findChild.assert_called_once_with(button_type, "qt_toolbar_ext_button")

    def test_missing_button_reports_the_expected_qt_object(self):
        toolbar = Mock()
        toolbar.findChild.return_value = None
        with self.assertRaisesRegex(RuntimeError, "toolbar overflow button 'qt_toolbar_ext_button' was not found"):
            toolbar_overflow(toolbar, object())
