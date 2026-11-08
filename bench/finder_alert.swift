// Clears a Finder alert that blocks new windows. A Finder AXDialog whose only
// button is OK is pressed and reported; any other dialog is left alone and the
// tool exits 2 so the caller stops. It prints only button names and counts,
// never the alert's text, which can name George's files.
//
//   finder_alert            exit 0: no dialog or an OK-only alert dismissed; 2: another dialog is open
import AppKit
import ApplicationServices

func attr(_ e: AXUIElement, _ a: String) -> CFTypeRef? {
    var v: CFTypeRef?
    AXUIElementCopyAttributeValue(e, a as CFString, &v)
    return v
}

func buttons(_ e: AXUIElement, _ depth: Int = 0) -> [AXUIElement] {
    if (attr(e, kAXRoleAttribute) as? String) == "AXButton" { return [e] }
    guard depth < 4 else { return [] }
    return (attr(e, kAXChildrenAttribute) as? [AXUIElement] ?? []).flatMap { buttons($0, depth + 1) }
}

guard let finder = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.finder").first else { exit(0) }
let app = AXUIElementCreateApplication(finder.processIdentifier)
let dialogs = (attr(app, kAXWindowsAttribute) as? [AXUIElement] ?? []).filter { (attr($0, kAXSubroleAttribute) as? String) == "AXDialog" }
var blocked = false
for dialog in dialogs {
    let found = buttons(dialog)
    let names = found.map { attr($0, kAXTitleAttribute) as? String ?? "" }
    if names == ["OK"] {
        let result = AXUIElementPerformAction(found[0], kAXPressAction as CFString)
        print("finder_alert: dismissed an OK-only Finder alert (AX result \(result.rawValue))")
    } else {
        print("finder_alert: a Finder dialog with buttons \(names) is open; left alone")
        blocked = true
    }
}
exit(blocked ? 2 : 0)
