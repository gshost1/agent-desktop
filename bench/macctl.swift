// Bench control and independent observation without AppleScript or System
// Events. Uses NSWorkspace/NSRunningApplication and the Accessibility grant the
// launching app already holds; it never asks for Automation.
//
//   macctl front                         {"app","pid","x","y"} of the frontmost app and the mouse
//   macctl activate <app>                bring <app> to the front
//   macctl quit <app>                    ask <app> to quit (no force) and wait up to 5 s for it to exit
//   macctl windows <app>                 JSON list of <app>'s window titles, front first
//   macctl menu <app> <menu> <item>      press <menu> > <item> in <app>'s menu bar
//   macctl selection <app>               JSON list of the selected row names in <app>'s front window
//   macctl raise <app> <title>           bring <app> and its window titled <title> to the front
//   macctl close-front-discard <app>     close <app>'s front window, discarding unsaved changes
//   macctl text <app>                    value of the first text area in <app>'s front window
//   macctl close-discard <app> <prefix>...  close <app>'s windows whose title starts with any
//                                        <prefix>, answering a save prompt with its discard button
//   macctl focused <app>                 JSON {role, subrole, value, actions, window} of <app>'s focused element
//   macctl set-focused <app> <value>     set AXValue of <app>'s focused element
//   macctl perform-focused <app> <action>  perform an AX action (AXConfirm, ...) on the focused element
//   macctl unfocus-focused <app>         set AXFocused false on the focused element
//   macctl key <app> <keycode> [cmd,shift,...]  post one key press to <app>'s process only
//   macctl keypid <pid> <keycode> [mods]  the same, to a pid
//   macctl owners                        JSON list of on-screen windows: pid, owner, layer, number, bounds
//   macctl pid-of <bundle id>            pids of running processes with that bundle id
import AppKit
import ApplicationServices
import Foundation

func fail(_ message: String) -> Never {
    FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
    exit(1)
}

func app(_ name: String) -> NSRunningApplication? {
    NSWorkspace.shared.runningApplications.first { $0.localizedName == name }
}

func attr(_ el: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(el, name as CFString, &value) == .success ? value : nil
}

func string(_ el: AXUIElement, _ name: String) -> String? { attr(el, name) as? String }
func children(_ el: AXUIElement) -> [AXUIElement] { (attr(el, "AXChildren") as? [AXUIElement]) ?? [] }
func elements(_ el: AXUIElement, _ name: String) -> [AXUIElement] { (attr(el, name) as? [AXUIElement]) ?? [] }

func axApp(_ name: String) -> AXUIElement {
    guard let running = app(name) else { fail("not running: \(name)") }
    let el = AXUIElementCreateApplication(running.processIdentifier)
    AXUIElementSetMessagingTimeout(el, 5)
    return el
}

func printJSON(_ value: Any) {
    let data = try! JSONSerialization.data(withJSONObject: value)
    print(String(data: data, encoding: .utf8)!)
}

func findAll(_ root: AXUIElement, depth: Int = 0, _ match: (AXUIElement) -> Bool) -> [AXUIElement] {
    if depth > 30 { return [] }
    var found = match(root) ? [root] : []
    for child in children(root) { found += findAll(child, depth: depth + 1, match) }
    return found
}

func press(_ el: AXUIElement) -> Bool { AXUIElementPerformAction(el, kAXPressAction as CFString) == .success }

func closeDiscarding(_ target: AXUIElement, _ window: AXUIElement) {
    if let close = attr(window, "AXCloseButton") { _ = press(close as! AXUIElement) }
    Thread.sleep(forTimeInterval: 0.5)
    let discard = ["Delete", "Delete Copy", "Don't Save", "Don\u{2019}t Save", "Revert Changes", "Discard"]
    let sheets = elements(target, "AXWindows").flatMap { w in children(w).filter { string($0, "AXRole") == "AXSheet" } }
    for sheet in sheets {
        if let button = findAll(sheet, { string($0, "AXRole") == "AXButton" && discard.contains(string($0, "AXTitle") ?? "") }).first {
            _ = press(button)
        }
    }
}

let args = Array(CommandLine.arguments.dropFirst())
guard let command = args.first else { fail("usage: macctl <command> ...") }

switch command {
case "front":
    let front = NSWorkspace.shared.frontmostApplication
    let mouse = NSEvent.mouseLocation
    printJSON(["app": front?.localizedName ?? "", "pid": front?.processIdentifier ?? 0,
               "x": Double(mouse.x), "y": Double(mouse.y)])
case "activate":
    guard args.count == 2, let running = app(args[1]) else { fail("activate <running app>") }
    running.activate()
    Thread.sleep(forTimeInterval: 0.3)
case "quit":
    guard args.count == 2 else { fail("quit <app>") }
    if let running = app(args[1]) {
        _ = running.terminate()
        for _ in 0..<50 where !running.isTerminated { Thread.sleep(forTimeInterval: 0.1) }
    }
case "windows":
    guard args.count == 2 else { fail("windows <app>") }
    printJSON(elements(axApp(args[1]), "AXWindows").map { string($0, "AXTitle") ?? "" })
case "menu":
    guard args.count == 4 else { fail("menu <app> <menu> <item>") }
    guard let bar = attr(axApp(args[1]), "AXMenuBar") else { fail("no menu bar") }
    let barEl = bar as! AXUIElement
    guard let top = children(barEl).first(where: { string($0, "AXTitle") == args[2] }) else { fail("no menu \(args[2])") }
    guard let item = findAll(top, { string($0, "AXTitle") == args[3] && string($0, "AXRole") == "AXMenuItem" }).first
    else { fail("no item \(args[3])") }
    if !press(item) { fail("press failed") }
case "selection":
    guard args.count == 2 else { fail("selection <app>") }
    guard let window = elements(axApp(args[1]), "AXWindows").first else { printJSON([]); break }
    let tables = findAll(window, { ["AXOutline", "AXTable"].contains(string($0, "AXRole") ?? "") })
    var names: [String] = []
    for table in tables {
        for row in elements(table, "AXSelectedRows") {
            let texts = findAll(row, { ["AXTextField", "AXStaticText"].contains(string($0, "AXRole") ?? "") })
            if let name = texts.compactMap({ string($0, "AXValue") }).first(where: { !$0.isEmpty }) { names.append(name) }
        }
    }
    printJSON(names)
case "close-discard":
    guard args.count >= 3 else { fail("close-discard <app> <title prefix>...") }
    guard app(args[1]) != nil else { break }
    let target = axApp(args[1])
    let prefixes = Array(args.dropFirst(2))
    for window in elements(target, "AXWindows") where prefixes.contains(where: { (string(window, "AXTitle") ?? "").hasPrefix($0) }) {
        closeDiscarding(target, window)
    }
case "close-front-discard":
    guard args.count == 2 else { fail("close-front-discard <app>") }
    guard app(args[1]) != nil else { break }
    let target = axApp(args[1])
    if let window = elements(target, "AXWindows").first { closeDiscarding(target, window) }
case "text":
    guard args.count == 2 else { fail("text <app>") }
    guard let window = elements(axApp(args[1]), "AXWindows").first,
          let area = findAll(window, { string($0, "AXRole") == "AXTextArea" }).first else { print(""); break }
    print(string(area, "AXValue") ?? "")
case "raise":
    guard args.count == 3, let running = app(args[1]) else { fail("raise <running app> <window title>") }
    running.activate()
    let target = axApp(args[1])
    guard let window = elements(target, "AXWindows").first(where: { string($0, "AXTitle") == args[2] }) else { fail("no window \(args[2])") }
    AXUIElementPerformAction(window, kAXRaiseAction as CFString)
    AXUIElementSetAttributeValue(window, kAXMainAttribute as CFString, kCFBooleanTrue)
    Thread.sleep(forTimeInterval: 0.3)
case "focused":
    guard args.count == 2 else { fail("focused <app>") }
    guard let focused = attr(axApp(args[1]), "AXFocusedUIElement") else { printJSON([:]); break }
    let el = focused as! AXUIElement
    var names: CFArray?
    AXUIElementCopyActionNames(el, &names)
    var window = ""
    if let w = attr(el, "AXWindow") { window = string(w as! AXUIElement, "AXTitle") ?? "" }
    var url = ""
    var cursor: AXUIElement? = el
    for _ in 0..<6 {
        guard let current = cursor else { break }
        if let u = attr(current, "AXURL") { url = (u as? URL)?.absoluteString ?? "\(u)"; break }
        cursor = attr(current, "AXParent").map { $0 as! AXUIElement }
    }
    printJSON(["role": string(el, "AXRole") ?? "", "subrole": string(el, "AXSubrole") ?? "",
               "value": string(el, "AXValue") ?? "", "actions": (names as? [String]) ?? [], "window": window,
               "url": url])
case "set-focused", "perform-focused", "unfocus-focused":
    guard args.count >= 2, let focused = attr(axApp(args[1]), "AXFocusedUIElement") else { fail("no focused element") }
    let el = focused as! AXUIElement
    let result: AXError
    switch command {
    case "set-focused": result = AXUIElementSetAttributeValue(el, kAXValueAttribute as CFString, args[2] as CFString)
    case "perform-focused": result = AXUIElementPerformAction(el, args[2] as CFString)
    default: result = AXUIElementSetAttributeValue(el, kAXFocusedAttribute as CFString, kCFBooleanFalse)
    }
    print(result.rawValue)
case "owners":
    let list = (CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]]) ?? []
    printJSON(list.map { w in ["pid": w[kCGWindowOwnerPID as String] ?? 0, "owner": w[kCGWindowOwnerName as String] ?? "",
                               "layer": w[kCGWindowLayer as String] ?? 0, "number": w[kCGWindowNumber as String] ?? 0,
                               "bounds": w[kCGWindowBounds as String] ?? [:]] })
case "pid-of":
    guard args.count == 2 else { fail("pid-of <bundle id>") }
    printJSON(NSRunningApplication.runningApplications(withBundleIdentifier: args[1]).map { Int($0.processIdentifier) })
case "key", "keypid":
    let target: pid_t?
    if command == "keypid" { target = Int32(args.count > 1 ? args[1] : "") } else { target = args.count > 1 ? app(args[1])?.processIdentifier : nil }
    guard args.count >= 3, let pid = target, let code = UInt16(args[2]) else { fail("key <app>|keypid <pid> <keycode> [mods]") }
    var flags = CGEventFlags()
    for m in (args.count > 3 ? args[3] : "").split(separator: ",") {
        switch m { case "cmd": flags.insert(.maskCommand); case "shift": flags.insert(.maskShift)
        case "alt": flags.insert(.maskAlternate); case "ctrl": flags.insert(.maskControl); default: break }
    }
    for down in [true, false] {
        guard let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else { fail("no event") }
        event.flags = flags
        event.postToPid(pid)
        Thread.sleep(forTimeInterval: 0.03)
    }
default:
    fail("unknown command \(command)")
}
