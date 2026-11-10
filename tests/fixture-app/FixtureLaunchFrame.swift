import AppKit

// Places the main fixture window at the frame the harness passes in
// AGENT_DESKTOP_FIXTURE_FRAME as "x,y,width,height" in global top-left screen
// points, the coordinate space `list-displays` and `list-windows` report. The
// value is all-or-nothing: exactly four finite numbers with a positive width
// and height, or the window keeps its default frame. The frame is cut to the
// visible area of the screen it lands on, so the menu bar and the Dock never
// move it. A frame saved by an earlier launch is restored when the window is
// created; applying the requested frame replaces it. Autosave is turned off,
// and every window-frame entry is removed from the fixture's defaults whenever
// they change, because SwiftUI re-arms the autosave later in a window's life.
// Only the fixture's own persistent domain is scanned, never the global one.
// A launch with a requested frame therefore leaves no saved frame behind. The
// harness verifies the result through `list-windows`, so no UI scripting is
// needed.
enum FixtureLaunchFrame {
    static let environmentKey = "AGENT_DESKTOP_FIXTURE_FRAME"
    static let mainWindowTitle = "AgentDesk Fixture"
    static let savedFramePrefix = "NSWindow Frame "
    private static var defaultsObserver: NSObjectProtocol?

    static func requested() -> NSRect? {
        guard let raw = ProcessInfo.processInfo.environment[environmentKey] else { return nil }
        return parse(raw)
    }

    static func parse(_ raw: String) -> NSRect? {
        let fields = raw.split(separator: ",", omittingEmptySubsequences: false)
        guard fields.count == 4 else { return nil }
        var values: [Double] = []
        for field in fields {
            guard let value = Double(field.trimmingCharacters(in: .whitespaces)), value.isFinite else {
                return nil
            }
            values.append(value)
        }
        guard values[2] > 0, values[3] > 0 else { return nil }
        return NSRect(x: values[0], y: values[1], width: values[2], height: values[3])
    }

    static func apply(_ topLeftFrame: NSRect, remainingAttempts: Int = 100) {
        let window = NSApp.windows.first { $0.title == mainWindowTitle && $0.isVisible }
        guard let window, let primary = NSScreen.screens.first else {
            if remainingAttempts > 0 {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                    apply(topLeftFrame, remainingAttempts: remainingAttempts - 1)
                }
            }
            return
        }
        stopFrameAutosave(window)
        removeSavedFrames()
        let cocoaFrame = NSRect(
            x: topLeftFrame.minX,
            y: primary.frame.maxY - topLeftFrame.minY - topLeftFrame.height,
            width: topLeftFrame.width,
            height: topLeftFrame.height
        )
        let screen = NSScreen.screens.first { $0.frame.intersects(cocoaFrame) } ?? primary
        let visibleFrame = cocoaFrame.intersection(screen.visibleFrame)
        window.setFrame(visibleFrame.isEmpty ? cocoaFrame : visibleFrame, display: true)
    }

    static func keepFramesOutOfDefaults() {
        removeSavedFrames()
        guard defaultsObserver == nil else { return }
        defaultsObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification, object: nil, queue: .main
        ) { _ in
            NSApp.windows.forEach(stopFrameAutosave)
            removeSavedFrames()
        }
    }

    private static func removeSavedFrames() {
        let defaults = UserDefaults.standard
        guard let domain = Bundle.main.bundleIdentifier,
              let saved = defaults.persistentDomain(forName: domain) else { return }
        for key in saved.keys where key.hasPrefix(savedFramePrefix) {
            defaults.removeObject(forKey: key)
        }
    }

    private static func stopFrameAutosave(_ window: NSWindow) {
        guard !window.frameAutosaveName.isEmpty else { return }
        window.setFrameAutosaveName("")
    }
}
