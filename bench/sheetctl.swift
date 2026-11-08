// Probe-only safety helper for probe_sheetfind_large.sh. Reads the sheet on one
// named window of one app, never descending into file-list containers
// (AXOutline, AXTable, AXList, AXBrowser, AXScrollArea), so it answers in a
// fixed, small number of reads however large the panel's folder is. It prints
// booleans and counts only, never names, titles or values.
//   sheetctl present  APP WINDOW            -> {"sheets":N}
//   sheetctl where-is APP WINDOW FOLDER     -> {"sheets":N,"where_found":B,"where_matches":B}
//   sheetctl expanded APP WINDOW            -> {"sheets":N,"expanded":B}
//   sheetctl toggle   APP WINDOW            -> {"sheets":N,"pressed":B}
//   sheetctl cancel   APP WINDOW            -> {"pressed":B,"cancelled":B}
import AppKit
import ApplicationServices

let skipped: Set<String> = ["AXOutline", "AXTable", "AXList", "AXBrowser", "AXScrollArea"]
let fileLists: Set<String> = ["AXOutline", "AXTable", "AXList", "AXBrowser"]

func attr(_ e: AXUIElement, _ a: String) -> CFTypeRef? {
    var v: CFTypeRef?
    return AXUIElementCopyAttributeValue(e, a as CFString, &v) == .success ? v : nil
}
func str(_ e: AXUIElement, _ a: String) -> String? { attr(e, a) as? String }
func kids(_ e: AXUIElement) -> [AXUIElement] { attr(e, kAXChildrenAttribute) as? [AXUIElement] ?? [] }
func emit(_ fields: [String: Any]) {
    let data = try! JSONSerialization.data(withJSONObject: fields, options: [.sortedKeys])
    print(String(data: data, encoding: .utf8)!)
}

let args = CommandLine.arguments
guard args.count >= 4 else { emit(["error": "usage"]); exit(2) }
let (command, appName, windowTitle) = (args[1], args[2], args[3])
guard let app = NSWorkspace.shared.runningApplications.first(where: { $0.localizedName == appName }) else {
    emit(["error": "no app"]); exit(1)
}
let axApp = AXUIElementCreateApplication(app.processIdentifier)
AXUIElementSetMessagingTimeout(axApp, 2.0)

func sheets() -> [AXUIElement] {
    let windows = attr(axApp, kAXWindowsAttribute) as? [AXUIElement] ?? []
    guard let window = windows.first(where: { str($0, kAXTitleAttribute) == windowTitle }) else { return [] }
    return kids(window).filter { str($0, kAXRoleAttribute) == "AXSheet" }
}

func first(_ e: AXUIElement, _ depth: Int, _ pred: (AXUIElement, String) -> Bool) -> AXUIElement? {
    let role = str(e, kAXRoleAttribute) ?? ""
    if pred(e, role) { return e }
    guard depth < 10, !skipped.contains(role) else { return nil }
    for k in kids(e) { if let hit = first(k, depth + 1, pred) { return hit } }
    return nil
}

func labelled(_ e: AXUIElement, _ label: String) -> Bool {
    if str(e, kAXTitleAttribute) == label || str(e, kAXDescriptionAttribute) == label { return true }
    if let t = attr(e, kAXTitleUIElementAttribute) {
        let titleElement = t as! AXUIElement
        return str(titleElement, kAXValueAttribute) == label || str(titleElement, kAXTitleAttribute) == label
    }
    return false
}

let all = sheets()
switch command {
case "present":
    emit(["sheets": all.count])
case "where-is":
    guard args.count >= 5 else { emit(["error": "usage"]); exit(2) }
    let popup = all.first.flatMap { s in
        first(s, 0) { e, role in ["AXPopUpButton", "AXComboBox", "AXMenuButton"].contains(role) && labelled(e, "Where:") }
    }
    let value = popup.flatMap { str($0, kAXValueAttribute) ?? str($0, kAXTitleAttribute) }?
        .replacingOccurrences(of: "\u{2068}", with: "").replacingOccurrences(of: "\u{2069}", with: "")
        .trimmingCharacters(in: .whitespaces)
    emit(["sheets": all.count, "where_found": popup != nil, "where_matches": value == args[4]])
case "expanded":
    let list = all.first.flatMap { s in
        first(s, 0) { e, role in
            fileLists.contains(role)
                || (role == "AXScrollArea" && kids(e).contains { fileLists.contains(str($0, kAXRoleAttribute) ?? "") })
        }
    }
    emit(["sheets": all.count, "expanded": list != nil])
case "toggle":
    let triangle = all.first.flatMap { s in first(s, 0) { _, role in role == "AXDisclosureTriangle" } }
    let pressed = triangle.map { AXUIElementPerformAction($0, kAXPressAction as CFString) == .success } ?? false
    emit(["sheets": all.count, "pressed": pressed])
case "cancel":
    let button = all.first.flatMap { s in first(s, 0) { e, role in role == "AXButton" && str(e, kAXTitleAttribute) == "Cancel" } }
    let pressed = button.map { AXUIElementPerformAction($0, kAXPressAction as CFString) == .success } ?? false
    var cancelled = all.isEmpty
    let until = Date().addingTimeInterval(10)
    while pressed && !cancelled && Date() < until {
        usleep(100_000)
        cancelled = sheets().isEmpty
    }
    emit(["pressed": pressed, "cancelled": cancelled])
default:
    emit(["error": "unknown command"]); exit(2)
}
