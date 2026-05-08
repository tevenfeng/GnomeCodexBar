import SwiftUI

@main
struct CodexBarApp: App {
    @StateObject private var reader = StatusReader()

    var body: some Scene {
        MenuBarExtra {
            PopoverContent(reader: reader)
        } label: {
            HStack(spacing: 4) {
                Text(reader.shortLabel)
                    .font(.system(size: 10, weight: .medium))
                Text(reader.summaryText)
            }
        }
        .menuBarExtraStyle(.window)

        Settings {
            EmptyView()
        }
    }
}
