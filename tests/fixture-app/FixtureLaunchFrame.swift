import AppKit

// Places the main fixture window at the frame the harness passes in
// AGENT_DESKTOP_FIXTURE_FRAME as "x,y,width,height" in global top-left screen
// points, the coordinate space `list-displays` and `list-windows` report. The
// frame is cut to the visible area of the screen it lands on, so the menu bar
// and the Dock never move it. The harness verifies the result through
// `list-windows`, so no UI scripting is needed to normalize the window.
enum FixtureLaunchFrame {
    static let environmentKey = "AGENT_DESKTOP_FIXTURE_FRAME"
    static let mainWindowTitle = "AgentDesk Fixture"

    static func requested() -> NSRect? {
        guard let raw = ProcessInfo.processInfo.environment[environmentKey] else { return nil }
        let parts = raw.split(separator: ",").compactMap { Double($0) }
        guard parts.count == 4, parts[2] > 0, parts[3] > 0 else { return nil }
        return NSRect(x: parts[0], y: parts[1], width: parts[2], height: parts[3])
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
}
