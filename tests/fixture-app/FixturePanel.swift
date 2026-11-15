import AppKit
import SwiftUI

// A floating utility panel, the window kind font, colour and inspector panels
// use: an NSPanel at the floating window level, owned by the fixture process
// beside its document window. Everything it changes is reported in the main
// window's readouts, so the harness verifies the panel's effects without
// trusting a command's own report.
//
// - panel-check: a checkbox; panel-check-status reads on/off.
// - panel-height / panel-width: linked fields. Editing the height sets the
//   width to 1.5 times it through the field's text-change delegate, the way an
//   image-size sheet keeps an aspect ratio, so panel-width-status proves the
//   application itself saw a value write, not only the field's cell.
// - panel-text: a text view whose selection panel-selection-status reports as
//   "location+length".
// - panel-commit: a field that hands its text to the model only when editing
//   ends, as form fields bound to a document do; panel-commit-status reads the
//   committed text, so a write the app never committed shows the old value.
// - panel-commit-text: a text view that commits the same way, read through
//   panel-commit-text-status.
final class PanelState: ObservableObject {
    static let shared = PanelState()
    @Published var checked = false
    @Published var width = "300"
    @Published var selection = "none"
    @Published var committed = "first"
    @Published var committedText = "first text"
}

final class FixturePanelController: NSObject, NSTextFieldDelegate, NSTextViewDelegate {
    static let shared = FixturePanelController()
    private var panel: NSPanel?
    private let height = NSTextField(string: "200")
    private let width = NSTextField(string: "300")
    private let commit = NSTextField(string: "first")
    private let commitText = NSTextView(frame: NSRect(x: 0, y: 0, width: 280, height: 40))

    func show() {
        if panel == nil { panel = makePanel() }
        panel?.orderFront(nil)
    }

    private func makePanel() -> NSPanel {
        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 320, height: 300),
            styleMask: [.titled, .closable, .utilityWindow],
            backing: .buffered,
            defer: false
        )
        panel.title = "Fixture Panel"
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = true
        panel.becomesKeyOnlyIfNeeded = true
        panel.isReleasedWhenClosed = false
        let check = NSButton(checkboxWithTitle: "Panel Check", target: self, action: #selector(toggled(_:)))
        check.setAccessibilityLabel("panel-check")
        height.setAccessibilityLabel("panel-height")
        height.delegate = self
        width.setAccessibilityLabel("panel-width")
        width.isEditable = false
        let text = NSTextView(frame: NSRect(x: 0, y: 0, width: 280, height: 40))
        text.string = "alpha beta gamma beta"
        text.setAccessibilityLabel("panel-text")
        text.delegate = self
        commit.setAccessibilityLabel("panel-commit")
        commit.delegate = self
        commitText.string = "first text"
        commitText.setAccessibilityLabel("panel-commit-text")
        commitText.delegate = self
        commitText.widthAnchor.constraint(equalToConstant: 280).isActive = true
        commitText.heightAnchor.constraint(equalToConstant: 40).isActive = true
        let stack = NSStackView(views: [check, height, width, text, commit, commitText])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        panel.contentView = stack
        if let screen = NSScreen.main?.visibleFrame {
            panel.setFrameTopLeftPoint(NSPoint(x: screen.maxX - 340, y: screen.maxY - 20))
        }
        return panel
    }

    @objc private func toggled(_ sender: NSButton) {
        PanelState.shared.checked = sender.state == .on
    }

    func controlTextDidEndEditing(_ notification: Notification) {
        guard notification.object as? NSTextField === commit else { return }
        PanelState.shared.committed = commit.stringValue
    }

    func textDidEndEditing(_ notification: Notification) {
        guard notification.object as? NSTextView === commitText else { return }
        PanelState.shared.committedText = commitText.string
    }

    func controlTextDidChange(_ notification: Notification) {
        guard notification.object as? NSTextField === height else { return }
        guard let value = Double(height.stringValue) else { return }
        width.stringValue = String(Int(value * 1.5))
        PanelState.shared.width = width.stringValue
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        guard let view = notification.object as? NSTextView else { return }
        let range = view.selectedRange()
        PanelState.shared.selection = "\(range.location)+\(range.length)"
    }
}

struct FloatingPanelControls: View {
    @ObservedObject private var state = PanelState.shared

    var body: some View {
        Button("Open Floating Panel") { FixturePanelController.shared.show() }
            .accessibilityLabel("open-floating-panel")
        StatusReadout(name: "panel-check-status", value: state.checked ? "on" : "off")
        StatusReadout(name: "panel-width-status", value: state.width)
        StatusReadout(name: "panel-selection-status", value: state.selection)
        StatusReadout(name: "panel-commit-status", value: state.committed)
        StatusReadout(name: "panel-commit-text-status", value: state.committedText)
    }
}
