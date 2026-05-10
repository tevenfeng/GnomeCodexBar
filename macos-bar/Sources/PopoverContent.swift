import SwiftUI

/// Main popover content: title + provider cards + refresh/options buttons
struct PopoverContent: View {
    @ObservedObject var reader: StatusReader
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // ── Title row ────────────────────────
            HStack {
                Text("Codex Bar")
                    .font(.system(size: 15, weight: .bold))
                    .foregroundColor(colorScheme == .dark ? .white : Color(red: 0.1, green: 0.1, blue: 0.1))
                Spacer()
                Button(action: refresh) {
                    Text("↻")
                        .font(.system(size: 16))
                        .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.4) : Color(red: 0.6, green: 0.6, blue: 0.6))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(
                            RoundedRectangle(cornerRadius: 4)
                                .fill(colorScheme == .dark ? Color.white.opacity(0.08) : Color.black.opacity(0.06))
                        )
                }
                .buttonStyle(.plain)

                Menu {
                    Button("Settings...") {
                        openSettings()
                    }
                    Divider()
                    Button("Quit Codex Bar") {
                        NSApplication.shared.terminate(nil)
                    }
                } label: {
                    Text("⋮")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.4) : Color(red: 0.6, green: 0.6, blue: 0.6))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(
                            RoundedRectangle(cornerRadius: 4)
                                .fill(colorScheme == .dark ? Color.white.opacity(0.08) : Color.black.opacity(0.06))
                        )
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
            }
            .padding(.bottom, 8)

            // ── Provider cards (stacked) ──────────
            let enabledProviders = reader.enabledProviders
            if !enabledProviders.isEmpty {
                let sel = reader.selectedProvider
                ForEach(Array(enabledProviders.enumerated()), id: \.element.id) { index, provider in
                    if index > 0 {
                        Rectangle()
                            .fill(colorScheme == .dark ? Color.white.opacity(0.08) : Color(red: 0.88, green: 0.88, blue: 0.88))
                            .frame(height: 1)
                            .padding(.vertical, 4)
                    }
                    ProviderCardView(
                        provider: provider,
                        isSelected: provider.providerId == sel,
                        updatedAt: reader.status?.updatedAt,
                        onSelect: {
                            reader.writeSelectedProvider(provider.providerId)
                        },
                        reader: reader
                    )
                }
            } else {
                Text("No data.\nRun \"codex-bar-cli daemon\" first.")
                    .font(.system(size: 12))
                    .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.6) : Color(red: 0.4, green: 0.4, blue: 0.4))
                    .padding(.vertical, 8)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
        .frame(width: 310)
        .background(
            RoundedRectangle(cornerRadius: 10)
                .fill(colorScheme == .dark
                      ? Color(red: 0.15, green: 0.15, blue: 0.15).opacity(0.95)
                      : Color(red: 0.96, green: 0.96, blue: 0.96))
        )
    }

    private func refresh() {
        DispatchQueue.global(qos: .userInitiated).async {
            let task = Process()
            task.executableURL = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".local/bin/codex-bar-cli")
            task.arguments = ["fetch"]
            try? task.run()
            task.waitUntilExit()

            DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                reader.readStatus()
            }
        }
    }

    private func openSettings() {
        openWindow(id: "settings")

        // CodexBar is an LSUIElement menu bar app. SwiftUI's openWindow creates
        // the settings window, but it does not always activate the app or bring
        // the new window above the currently focused application.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            NSApplication.shared.activate(ignoringOtherApps: true)

            if let settingsWindow = NSApplication.shared.windows.first(where: { $0.title == "Codex Bar Settings" }) {
                settingsWindow.center()
                settingsWindow.makeKeyAndOrderFront(nil)
                settingsWindow.orderFrontRegardless()
            }
        }
    }
}
