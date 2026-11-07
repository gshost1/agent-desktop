// Prints, for each control in the front window's sheet of <pid>, the AX error
// code of the reads the actionability gate depends on: AXWindow, AXExpanded,
// AXPosition, AXSize, AXHidden. Read-only; no actions are performed.
import ApplicationServices
import Foundation

let pid = pid_t(CommandLine.arguments[1])!
let app = AXUIElementCreateApplication(pid)
AXUIElementSetMessagingTimeout(app, 3)

func read(_ el: AXUIElement, _ attr: String) -> (AXError, CFTypeRef?) {
    var v: CFTypeRef?
    let e = AXUIElementCopyAttributeValue(el, attr as CFString, &v)
    return (e, v)
}
func str(_ el: AXUIElement, _ attr: String) -> String {
    let (e, v) = read(el, attr)
    return e == .success ? String(describing: v!) : "err\(e.rawValue)"
}
func children(_ el: AXUIElement) -> [AXUIElement] {
    let (e, v) = read(el, "AXChildren")
    return e == .success ? (v as? [AXUIElement] ?? []) : []
}
let controls: Set<String> = ["AXPopUpButton", "AXComboBox", "AXButton", "AXMenuButton", "AXDisclosureTriangle", "AXCheckBox", "AXTextField"]
func walk(_ el: AXUIElement, _ depth: Int) {
    if depth > 40 { return }
    let role = str(el, "AXRole")
    if controls.contains(role) {
        var row: [String: String] = ["role": role, "name": str(el, "AXTitle"), "desc": str(el, "AXDescription")]
        for a in ["AXWindow", "AXExpanded", "AXPosition", "AXSize", "AXHidden", "AXEnabled"] {
            let (e, v) = read(el, a)
            row[a] = e == .success ? (a == "AXWindow" ? "ok" : String(describing: v!).prefix(40).description) : "err\(e.rawValue)"
        }
        var names: CFArray?
        AXUIElementCopyAttributeNames(el, &names)
        row["has_expanded"] = ((names as? [String]) ?? []).contains("AXExpanded") ? "yes" : "no"
        print(String(data: try! JSONSerialization.data(withJSONObject: row), encoding: .utf8)!)
    }
    for c in children(el) { walk(c, depth + 1) }
}
let (e, w) = read(app, "AXFocusedWindow")
guard e == .success else { print("no focused window \(e.rawValue)"); exit(1) }
let win = w as! AXUIElement
let sheets = children(win).filter { str($0, "AXRole") == "AXSheet" }
print("sheets: \(sheets.count)")
for s in sheets { walk(s, 0) }
