import SwiftUI

@main
struct CodexBarApp: App {
    @StateObject private var reader = StatusReader()

    var body: some Scene {
        MenuBarExtra {
            PopoverContent(reader: reader)
        } label: {
            Text("\(reader.shortLabel) \(reader.summaryText)")
                .font(.system(size: 12, weight: .medium))
        }
        .menuBarExtraStyle(.window)

        Settings {
            EmptyView()
        }
    }
}
