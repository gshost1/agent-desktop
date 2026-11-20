"""A fullscreen GTK4 window on one monitor that logs every pointer event it receives.

Usage: pointer_probe_gtk4.py <log.jsonl> <connector>
Each line is one JSON object: the monitor it went fullscreen on, then every
press, release, motion, scroll and drag in window coordinates, which equal the
monitor's logical coordinates once the window is fullscreen.
"""

import json
import os
import sys

os.environ["GTK_USE_PORTAL"] = "0"
import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, Gtk


def main(argv):
    if len(argv) != 2:
        raise ValueError("expected the JSON-lines log path and a monitor connector")
    log = open(argv[0], "a", buffering=1)
    connector = argv[1]

    def emit(**fields):
        log.write(json.dumps(fields) + "\n")

    app = Gtk.Application(
        application_id="dev.agentdesktop.PointerProbe." + connector.replace("-", "_"),
        flags=Gio.ApplicationFlags.NON_UNIQUE,
    )

    def pressed(gesture, count, x, y):
        state = gesture.get_current_event_state()
        emit(ev="press", n=count, x=x, y=y, button=gesture.get_current_button(),
             ctrl=bool(state & Gdk.ModifierType.CONTROL_MASK),
             t=gesture.get_current_event_time())

    def scrolled(_controller, dx, dy):
        emit(ev="scroll", dx=dx, dy=dy)
        return True

    def activate(app):
        window = Gtk.ApplicationWindow(application=app, title="pointer probe " + connector)
        area = Gtk.DrawingArea(hexpand=True, vexpand=True)
        window.set_child(area)
        click = Gtk.GestureClick(button=0)
        click.connect("pressed", pressed)
        click.connect("released", lambda g, n, x, y: emit(
            ev="release", n=n, t=g.get_current_event_time()))
        area.add_controller(click)
        motion = Gtk.EventControllerMotion()
        motion.connect("motion", lambda _c, x, y: emit(ev="motion", x=x, y=y))
        area.add_controller(motion)
        scroll = Gtk.EventControllerScroll(flags=Gtk.EventControllerScrollFlags.BOTH_AXES)
        scroll.connect("scroll", scrolled)
        area.add_controller(scroll)
        drag = Gtk.GestureDrag()
        drag.connect("drag-begin", lambda _g, x, y: emit(ev="drag-begin", x=x, y=y))
        drag.connect("drag-end", lambda _g, dx, dy: emit(ev="drag-end", dx=dx, dy=dy))
        area.add_controller(drag)
        monitors = Gdk.Display.get_default().get_monitors()
        chosen = next(
            (monitors.get_item(i) for i in range(monitors.get_n_items())
             if monitors.get_item(i).get_connector() == connector),
            None,
        )
        if chosen is None:
            emit(ev="error", reason="no monitor " + connector)
            app.quit()
            return
        window.fullscreen_on_monitor(chosen)
        window.present()

        def report():
            surface = window.get_surface()
            on = Gdk.Display.get_default().get_monitor_at_surface(surface) if surface else None
            emit(ev="state", fullscreen=window.is_fullscreen(),
                 on=on.get_connector() if on else None,
                 width=window.get_width(), height=window.get_height())
            return True

        GLib.timeout_add(500, report)

    app.connect("activate", activate)
    app.run([])


if __name__ == "__main__":
    main(sys.argv[1:])
