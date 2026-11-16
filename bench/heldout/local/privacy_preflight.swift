// Privacy preflight for a pass on a personal Mac. Prints one JSON object and exits 0 when the pass may start,
// 1 when it must not. Reads only the running-application list, the https handlers Launch Services knows and the
// window server's owner names and layers (no titles, no pixels, no Accessibility, no Apple Events).
//
// It refuses while a mail, messages, photos or notes app or any web browser (an app registered for https) runs,
// or while Finder shows a window. Every other regular app is recorded, never quit. HO_PREFLIGHT_ALLOW (comma-separated
// bundle ids) names apps that register for https but are not browsers, such as the host's agent console.
import AppKit
import CoreGraphics

let personal: Set<String> = ["com.apple.mail", "com.apple.MobileSMS", "com.apple.Photos", "com.apple.Notes"]
var ignore: Set<String> = ["com.apple.finder"]
for id in (ProcessInfo.processInfo.environment["HO_PREFLIGHT_ALLOW"] ?? "").split(separator: ",") {
    ignore.insert(String(id))
}
var browsers = Set<String>()
if let url = URL(string: "https://example.com") {
    for app in NSWorkspace.shared.urlsForApplications(toOpen: url) {
        if let id = Bundle(url: app)?.bundleIdentifier { browsers.insert(id) }
    }
}
var running: [String] = []
var refused: [String] = []
for app in NSWorkspace.shared.runningApplications where app.activationPolicy == .regular {
    let id = app.bundleIdentifier ?? ""
    let name = app.localizedName ?? id
    running.append(name)
    if ignore.contains(id) { continue }
    if personal.contains(id) { refused.append("\(name) is running") }
    else if browsers.contains(id) { refused.append("\(name) (a web browser) is running") }
}
var finderWindows = 0
if let list = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] {
    for w in list where (w[kCGWindowOwnerName as String] as? String) == "Finder"
        && (w[kCGWindowLayer as String] as? Int) == 0 {
        let b = w[kCGWindowBounds as String] as? [String: Double] ?? [:]
        if (b["Width"] ?? 0) > 100 && (b["Height"] ?? 0) > 100 { finderWindows += 1 }
    }
}
if finderWindows > 0 { refused.append("Finder shows \(finderWindows) window(s)") }
let out: [String: Any] = ["ok": refused.isEmpty, "refused": refused, "running": running.sorted(),
                          "finder_windows": finderWindows]
if let data = try? JSONSerialization.data(withJSONObject: out, options: [.sortedKeys]),
   let text = String(data: data, encoding: .utf8) { print(text) }
exit(refused.isEmpty ? 0 : 1)
