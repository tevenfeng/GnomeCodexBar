import SwiftUI

@main
struct CodexBarApp: App {
    @StateObject private var reader = StatusReader()
    @State private var showSettingsWindow = false

    var body: some Scene {
        MenuBarExtra {
            PopoverContent(reader: reader)
        } label: {
            Text("\(reader.shortLabel) \(reader.summaryText)")
                .font(.system(size: 12, weight: .medium))
        }
        .menuBarExtraStyle(.window)

        Window("Codex Bar Settings", id: "settings") {
            SettingsView(reader: reader)
        }
        .windowStyle(.titleBar)
        .windowResizability(.contentSize)
        .defaultPosition(.center)
    }
}
