import SwiftUI

// A lazily built list: SwiftUI creates a row, and its accessibility element,
// only when the row scrolls near the viewport. A snapshot or a plain `find`
// sees the first rows only; `find --scroll` has to page the list to reach
// the rest. The E2E harness reads it through the labels below.

struct LazyListCard: View {
    @State private var pressed = "none"

    var body: some View {
        Card(title: "Lazy List") {
            StatusReadout(name: "lazy-pressed", value: pressed)
            ScrollView {
                LazyVStack(alignment: .leading) {
                    ForEach(1...400, id: \.self) { i in
                        Button("Lazy Row \(i)") { pressed = String(i) }
                            .accessibilityLabel("lazy-row-\(i)")
                    }
                }
            }
            .frame(width: 200, height: 160)
            .accessibilityLabel("lazy-list")
        }
    }
}
