from __future__ import annotations

import base64
import subprocess
from contextlib import contextmanager

from oracle import atspi_raw, pixels, x11
from runner import apps
from runner.helper import one, require
from runner.legs import implementation
from runner.verdict import LegContext
from scenarios.headed_actions import assert_ok, live_root, ref_for, restore_text, window_of
from scenarios.pointer import frame_extents


def non_blank(image: pixels.Image) -> bool:
    first = image.rgb[:3]
    return any(image.rgb[offset : offset + 3] != first for offset in range(3, len(image.rgb), 3))


def capture_bytes(envelope) -> tuple[bytes, str]:
    data = assert_ok(envelope)
    payload = data.get("data")
    if not payload:
        raise AssertionError(f"{envelope.argv} returned no image data")
    return base64.b64decode(payload), data.get("format") or "png"


def decode(data: bytes, image_format: str) -> pixels.Image:
    if image_format.lower() in ("xwd", "x-window-dump"):
        return pixels.decode_xwd(data)
    return pixels.decode_png(data)


def _x11_id(ctx: LegContext, window: str) -> str:
    return x11.xid(ctx.tier, window)


def _xwd(ctx: LegContext, x11_window: str | None) -> bytes:
    process = subprocess.run(
        ["xwd", *(["-id", x11_window] if x11_window else ["-root"]), "-silent"],
        capture_output=True,
        env=ctx.tier.env,
        timeout=10,
        check=False,
    )
    if process.returncode != 0 or not process.stdout:
        raise AssertionError("xwd returned no bytes")
    return process.stdout


def _document(ctx, app):
    return one(live_root(ctx.tier, app), role=61).text()


@contextmanager
def _editor(ctx: LegContext):
    app = apps.launch(ctx.tier, "Text Editor")
    try:
        window = window_of(app)
        initial = _document(ctx, app)
        ctx.before("atspi.document_text", initial)
        try:
            assert_ok(ctx.product(["type", ref_for(ctx, window, role="textfield"), "capture baseline"]))
            require(atspi_raw.poll(
                lambda: "capture baseline" in _document(ctx, app), 6.0),
                "capture text did not reach the document")
            ctx.after("atspi.document_text", _document(ctx, app))
            yield app, window
        finally:
            restore_text(ctx, app, initial, read=lambda: _document(ctx, app))
    finally:
        apps.stop(app)


def _oracle_image(ctx, window=None):
    image = pixels.decode_xwd(_xwd(ctx, _x11_id(ctx, window) if window else None))
    if window:
        left, right, top, bottom = frame_extents(ctx.tier, window)
        width, height = image.width - left - right, image.height - top - bottom
        require(min(left, right, top, bottom) >= 0 and min(width, height) > 0,
                "the independently reported shadow does not fit the XWD image")
        return image.crop((left, top, width, height))
    return image


def _product_image(ctx, window):
    payload, image_format = capture_bytes(ctx.product(["screenshot", "--window-id", window]))
    return decode(payload, image_format)


def _matches(product, oracle):
    require(pixels.similar(product, oracle, tolerance=0.05),
            "the product capture does not match independent xwd pixels")


@implementation("capture.window_pixels_match_xwd")
def window_pixels_match_xwd(ctx: LegContext) -> None:
    with _editor(ctx) as (app, window):
        before = _oracle_image(ctx, window)
        ctx.before("pixels.window_dimensions", (before.width, before.height))
        product = _product_image(ctx, window)
        after = _oracle_image(ctx, window)
        ctx.after("pixels.window_dimensions", (after.width, after.height))
        _matches(product, after)
        final = _oracle_image(ctx, window)
        ctx.restored("pixels.window_dimensions", (final.width, final.height))
