// BenchFront: the harness-owned app that holds the front at task start. It opens no window, reads nothing and
// takes no input; it only becomes the active app when the harness opens it, so task apps launched in the
// background stay behind it until a tool brings one forward.
import AppKit

let app = NSApplication.shared
app.setActivationPolicy(.regular)
app.activate(ignoringOtherApps: true)
app.run()
