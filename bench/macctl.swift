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
    let discard = ["Delete", "Don't Save", "Don\u{2019}t Save", "Revert Changes"]
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
default:
    fail("unknown command \(command)")
}
