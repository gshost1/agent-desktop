import base64
import unittest
from contextlib import contextmanager, ExitStack, nullcontext
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from runner import legs, verdict
from runner.product import Envelope
from runner.verdict import LegContext
from scenarios import capture, headed_actions


class CaptureScenarioTests(unittest.TestCase):
    @contextmanager
    def fixture(self):
        tier = SimpleNamespace(name="x11-openbox", runtime_dir=Path("/private/test"), env={})
        contract = legs.leg_by_id("capture.window_pixels_match_xwd")
        ctx = LegContext(tier, "headless", contract, member_reader=lambda _: frozenset())
        app = SimpleNamespace(windows=["w-editor"])
        initial = capture.pixels.Image(2, 2, bytes((1, 2, 3, 4, 5, 6) * 2))
        state = {"raw_reads": 0}
        def xwd(*args):
            state["raw_reads"] += 1
            return capture.pixels.encode_png(initial)
        def product(tier, argv, **kw):
            data = {"data": base64.b64encode(capture.pixels.encode_png(initial)).decode(), "format": "png"}
            return Envelope(True, data, None, "{}", 0, argv)
        with ExitStack() as stack:
            replacements = [
                (capture, "_editor", lambda _: nullcontext((app, "w-editor"))),
                (capture.apps, "stop", lambda _: None),
                (capture, "_x11_id", lambda _, window: "0x1"),
                (capture, "_xwd", xwd),
                (capture, "frame_extents", lambda *a: (0, 0, 0, 0)),
                (capture.pixels, "decode_xwd", capture.pixels.decode_png),
            ]
            for owner, name, value in replacements:
                stack.enter_context(patch.object(owner, name, value, create=True))
            stack.enter_context(patch("runner.verdict.run_product", side_effect=product))
            yield ctx, state

    def test_window_oracle_excludes_independently_reported_client_shadows(self):
        image = capture.pixels.Image(5, 6, bytes(range(90)))
        ctx = SimpleNamespace(tier=SimpleNamespace())
        with patch.object(capture, "_xwd", return_value=b"xwd"), \
            patch.object(capture, "_x11_id", return_value="0x1"), \
            patch.object(capture.pixels, "decode_xwd", return_value=image), \
            patch.object(capture, "frame_extents", return_value=(1, 1, 2, 1), create=True):
            result = capture._oracle_image(ctx, "w-editor")
        self.assertEqual((result.width, result.height), (3, 3))
        self.assertEqual(result.rgb, image.rgb[33:42] + image.rgb[48:57] + image.rgb[63:72])

    def test_window_capture_observations_include_independent_final_reads(self):
        with self.fixture() as (ctx, state):
            capture.window_pixels_match_xwd(ctx)
            self.assertEqual(verdict.evaluate(ctx), [])
            self.assertGreaterEqual(state["raw_reads"], 2)

    def test_capture_restores_empty_and_multiline_drafts_even_when_capture_or_typing_fails(self):
        for initial in ("", "saved\ndraft"):
            for failure in (None, "capture", "type"):
                with self.subTest(initial=initial, failure=failure):
                    document, commands = [initial], []
                    tier = SimpleNamespace(runtime_dir=Path("/private/test"))
                    ctx = LegContext(tier, "headless", legs.leg_by_id("capture.window_pixels_match_xwd"),
                                     member_reader=lambda _: frozenset())
                    app = SimpleNamespace(windows=["w-editor"])

                    def product(tier, argv, **kw):
                        commands.append(argv)
                        if argv[0] == "type":
                            document[0] += argv[-1]
                            if failure == "type":
                                raise RuntimeError("typing failed after writing")
                        elif argv[0] == "clear":
                            document[0] = ""
                        elif argv[0] == "set-value":
                            document[0] = argv[-1]
                        return Envelope(True, {}, None, "{}", 0, argv)

                    with ExitStack() as stack:
                        stack.enter_context(patch.object(capture.apps, "launch", return_value=app))
                        stop = stack.enter_context(patch.object(capture.apps, "stop"))
                        stack.enter_context(patch.object(capture, "_document", side_effect=lambda *_: document[0]))
                        for module in (capture, headed_actions):
                            stack.enter_context(patch.object(module, "ref_for", return_value="@s:e1"))
                            stack.enter_context(patch.object(module, "live_root", return_value=app))
                        stack.enter_context(patch.object(capture.atspi_raw, "poll", side_effect=lambda read, *a, **kw: read()))
                        stack.enter_context(patch("runner.verdict.run_product", side_effect=product))
                        def run():
                            with capture._editor(ctx):
                                if failure == "capture":
                                    raise RuntimeError("capture failed")
                        if failure:
                            with self.assertRaises(RuntimeError):
                                run()
                        else:
                            run()
                            self.assertEqual(verdict.evaluate(ctx), [])
                        stop.assert_called_once_with(app)
                    self.assertEqual(document[0], initial)
                    self.assertEqual(commands[-1], ["set-value", "@s:e1", initial] if initial else ["clear", "@s:e1"])
