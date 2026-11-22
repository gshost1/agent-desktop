import AppKit
import CoreGraphics
import Foundation

let pid = Int32(CommandLine.arguments[1])!
let front = NSWorkspace.shared.frontmostApplication!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as! [[String: Any]]
let order = windows.filter {
    ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0
}.compactMap { $0[kCGWindowNumber as String] as? Int }
let state: [String: Any] = ["frontmost": front.localizedName ?? "", "pid": front.processIdentifier, "window_order": order]
let data = try JSONSerialization.data(withJSONObject: state, options: [.sortedKeys])
print(String(data: data, encoding: .utf8)!)
