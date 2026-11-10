// usage: winwatch <bundle id> <timeout s>; seconds from start to: process seen, finishedLaunching, first layer-0 CG window of its pid, first AX window
import Cocoa
import ApplicationServices
let bid = CommandLine.arguments[1]
let timeout = Double(CommandLine.arguments[2]) ?? 10
let t0 = Date()
var cg: Double? = nil, ax: Double? = nil, pidSeen: Double? = nil, fin: Double? = nil
while Date().timeIntervalSince(t0) < timeout && (cg == nil || ax == nil || fin == nil) {
  let el = Date().timeIntervalSince(t0)
  if let app = NSRunningApplication.runningApplications(withBundleIdentifier: bid).first {
    let pid = app.processIdentifier
    if pidSeen == nil { pidSeen = el }
    if fin == nil && app.isFinishedLaunching { fin = el }
    if cg == nil, let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] {
      if list.contains(where: { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }) { cg = el }
    }
    if ax == nil {
      let a = AXUIElementCreateApplication(pid)
      var v: CFTypeRef?
      if AXUIElementCopyAttributeValue(a, kAXWindowsAttribute as CFString, &v) == .success, let arr = v as? [AnyObject], !arr.isEmpty { ax = el }
    }
  }
  RunLoop.current.run(until: Date().addingTimeInterval(0.01))
}
func f(_ x: Double?) -> String { x.map { String(format: "%.2f", $0) } ?? "-" }
print("pid=\(f(pidSeen)) finished=\(f(fin)) cg_window=\(f(cg)) ax_window=\(f(ax))")
