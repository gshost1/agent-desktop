import AppKit
import SwiftUI

// Controls whose Return does something besides ending the edit, so the harness
// can prove a value write never fires them. Every count is reported in the main
// window, independent of the command's own report.
//
// AppKit, in the "Fixture Form" window:
// - form-plain: a field with no action that commits on end editing
//   (form-plain-status), in a window whose "Form Default" button answers Return
//   (form-default-count).
// - form-action: a field whose action runs on Return (form-action-count).
// - form-search: a search field whose action runs on Return (form-search-count).
// SwiftUI, in the main window:
// - swift-submit: a TextField with onSubmit (swift-submit-count) beside a
//   default-action button (swift-default-count).
final class FormState: ObservableObject {
    static let shared = FormState()
    @Published var defaultCount = 0
    @Published var actionCount = 0
    @Published var searchCount = 0
    @Published var plain = "plain"
    @Published var swiftSubmitCount = 0
    @Published var swiftDefaultCount = 0
}

final class FixtureFormController: NSObject, NSTextFieldDelegate {
    static let shared = FixtureFormController()
    private var window: NSWindow?
    private let plain = NSTextField(string: "plain")

    func show() {
        if window == nil { window = makeWindow() }
        window?.orderFront(nil)
    }

    private func makeWindow() -> NSWindow {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 320, height: 220),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "Fixture Form"
        window.isReleasedWhenClosed = false
        plain.setAccessibilityLabel("form-plain")
        plain.delegate = self
        let action = NSTextField(string: "action")
        action.setAccessibilityLabel("form-action")
        action.target = self
        action.action = #selector(fieldAction(_:))
        let search = NSSearchField(string: "")
        search.setAccessibilityLabel("form-search")
        search.target = self
        search.action = #selector(searchAction(_:))
        search.sendsSearchStringImmediately = false
        search.sendsWholeSearchString = true
        let submit = NSButton(title: "Form Default", target: self, action: #selector(defaultAction(_:)))
        submit.keyEquivalent = "\r"
        submit.setAccessibilityLabel("form-default")
        let stack = NSStackView(views: [plain, action, search, submit])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        window.contentView = stack
        window.defaultButtonCell = submit.cell as? NSButtonCell
        if let screen = NSScreen.main?.visibleFrame {
            window.setFrameTopLeftPoint(NSPoint(x: screen.maxX - 340, y: screen.maxY - 360))
        }
        return window
    }

    @objc private func defaultAction(_ sender: NSButton) { FormState.shared.defaultCount += 1 }
    @objc private func fieldAction(_ sender: NSTextField) { FormState.shared.actionCount += 1 }
    @objc private func searchAction(_ sender: NSSearchField) { FormState.shared.searchCount += 1 }

    func controlTextDidEndEditing(_ notification: Notification) {
        guard notification.object as? NSTextField === plain else { return }
        FormState.shared.plain = plain.stringValue
    }
}

struct FixtureFormControls: View {
    @ObservedObject private var state = FormState.shared
    @State private var swiftText = "swift"

    var body: some View {
        Button("Open Form Window") { FixtureFormController.shared.show() }
            .accessibilityLabel("open-form-window")
        TextField("Swift Submit", text: $swiftText)
            .accessibilityLabel("swift-submit")
            .onSubmit { state.swiftSubmitCount += 1 }
            .frame(width: 160)
        Button("Swift Default") { state.swiftDefaultCount += 1 }
            .keyboardShortcut(.defaultAction)
            .accessibilityLabel("swift-default")
        StatusReadout(name: "form-default-count", value: "\(state.defaultCount)")
        StatusReadout(name: "form-action-count", value: "\(state.actionCount)")
        StatusReadout(name: "form-search-count", value: "\(state.searchCount)")
        StatusReadout(name: "form-plain-status", value: state.plain)
        StatusReadout(name: "swift-submit-count", value: "\(state.swiftSubmitCount)")
        StatusReadout(name: "swift-default-count", value: "\(state.swiftDefaultCount)")
    }
}
