// Samples the frontmost app, its focused element and the cursor position every
// 100 ms and prints one JSON line per sample. Reads AX directly, so it needs no
// Automation (System Events) permission. Build: swiftc -O bench/focus_probe.swift -o bench/out/focus_probe
import AppKit
import ApplicationServices

func string(_ element: AXUIElement, _ attribute: String) -> String {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success else { return "" }
    return (value as? String) ?? ""
}

func sample() -> [String: Any] {
    let point = NSEvent.mouseLocation
    var out: [String: Any] = ["x": Int(point.x), "y": Int(point.y), "t": Date().timeIntervalSince1970]
    guard let app = NSWorkspace.shared.frontmostApplication else { out["front"] = "?"; out["focus"] = "?"; return out }
    out["front"] = app.localizedName ?? "?"
    let axApp = AXUIElementCreateApplication(app.processIdentifier)
    AXUIElementSetMessagingTimeout(axApp, 0.5)
    var focused: CFTypeRef?
    if AXUIElementCopyAttributeValue(axApp, kAXFocusedUIElementAttribute as CFString, &focused) == .success,
       let focused, CFGetTypeID(focused) == AXUIElementGetTypeID() {
        let element = focused as! AXUIElement
        out["focus"] = string(element, kAXRoleAttribute) + ":" + string(element, kAXDescriptionAttribute)
    } else {
        out["focus"] = "?"
    }
    return out
}

let once = CommandLine.arguments.contains("--once")
repeat {
    if let data = try? JSONSerialization.data(withJSONObject: sample()), let line = String(data: data, encoding: .utf8) {
        print(line)
        fflush(stdout)
    }
    if !once { usleep(100_000) }
} while !once
