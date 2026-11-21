"""Native GTK4 scroll viewport with checkable markers."""

import json
import os
import sys
from pathlib import Path

os.environ["GTK_USE_PORTAL"] = "0"
import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib, Gtk


def main(argv):
    if len(argv) != 1:
        raise ValueError("expected the JSON-lines log path")
    GLib.set_prgname("zoo_scroll.py")
    GLib.set_application_name("Zoo Scroll")
    Gtk.init()
    loop = GLib.MainLoop()
    window = Gtk.Window(title="Zoo Scroll")
    window.set_default_size(360, 180)
    window.set_resizable(False)
    window.connect("close-request", lambda *_: loop.quit() or False)
    scroll = Gtk.ScrolledWindow()
    scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.ALWAYS)
    scroll.set_min_content_height(180)
    scroll.set_max_content_height(180)
    scroll.set_propagate_natural_height(True)
    box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    box.set_margin_bottom(40)
    for label, checked in (("Upper marker", True), ("Lower marker", False)):
        if not checked:
            spacer = Gtk.Box()
            spacer.set_size_request(-1, 140)
            box.append(spacer)
        marker = Gtk.CheckButton(label=label)
        marker.set_size_request(-1, 40)
        marker.set_active(checked)
        box.append(marker)
    scroll.set_child(box)
    window.set_child(scroll)
    window.present()
    Path(argv[0]).write_text(json.dumps({"widget": "zoo-scroll", "event": "ready"}) + "\n")
    loop.run()


if __name__ == "__main__":
    main(sys.argv[1:])
