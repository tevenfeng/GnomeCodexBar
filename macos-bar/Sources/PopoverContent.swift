import SwiftUI
import Darwin

/// Main popover content: title + provider cards + refresh/options buttons
struct PopoverContent: View {
    @ObservedObject var reader: StatusReader
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.openWindow) private var openWindow
    @State private var isRefreshing = false
    @State private var refreshError: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // ── Title row ────────────────────────
            HStack {
                Text("Codex Bar")
                    .font(.system(size: 15, weight: .bold))
                    .foregroundColor(colorScheme == .dark ? .white : Color(red: 0.1, green: 0.1, blue: 0.1))
                Spacer()
                Button(action: refresh) {
                    Text(isRefreshing ? "…" : "↻")
                        .font(.system(size: 16))
                        .foregroundColor(colorScheme == .dark ? Color.white.opacity(isRefreshing ? 0.7 : 0.4) : Color(red: 0.6, green: 0.6, blue: 0.6))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(
                            RoundedRectangle(cornerRadius: 4)
                                .fill(colorScheme == .dark ? Color.white.opacity(0.08) : Color.black.opacity(0.06))
                        )
                }
                .buttonStyle(.plain)
                .disabled(isRefreshing)

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

            if let refreshError {
                Text(refreshError)
                    .font(.system(size: 11))
                    .foregroundColor(Color(red: 0.8, green: 0.2, blue: 0.2))
                    .padding(.bottom, 8)
            }

            // ── Provider cards (stacked) ──────────
            let enabledProviders = reader.enabledProviders
            if !enabledProviders.isEmpty {
                let effectiveSelected = reader.activeProvider?.providerId
                ForEach(Array(enabledProviders.enumerated()), id: \.element.id) { index, provider in
                    if index > 0 {
                        Rectangle()
                            .fill(colorScheme == .dark ? Color.white.opacity(0.08) : Color(red: 0.88, green: 0.88, blue: 0.88))
                            .frame(height: 1)
                            .padding(.vertical, 4)
                    }
                    ProviderCardView(
                        provider: provider,
                        isSelected: provider.providerId == effectiveSelected,
                        updatedAt: reader.status?.updatedAt,
                        onSelect: {
                            reader.writeSelectedProvider(provider.providerId)
                        },
                        reader: reader
                    )
                }
            } else {
                Text(reader.status == nil
                     ? "No data.\nRun \"codex-bar-cli daemon\" first."
                     : "No providers enabled.\nEnable providers in Settings.")
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
        guard !isRefreshing else { return }
        isRefreshing = true
        refreshError = nil

        DispatchQueue.global(qos: .userInitiated).async {
            let error = runRefreshCommand()

            DispatchQueue.main.async {
                isRefreshing = false
                if let error {
                    refreshError = error
                    reader.readStatus()
                } else {
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                        reader.readStatus()
                    }
                }
            }
        }
    }

    private func runRefreshCommand() -> String? {
        let candidates: [(URL, [String])] = [
            (FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/bin/codex-bar-cli"), ["fetch"]),
            (URL(fileURLWithPath: "/opt/homebrew/bin/codex-bar-cli"), ["fetch"]),
            (URL(fileURLWithPath: "/usr/local/bin/codex-bar-cli"), ["fetch"]),
            (URL(fileURLWithPath: "/usr/bin/env"), ["codex-bar-cli", "fetch"]),
        ]
        let timeout: TimeInterval = 30

        for (executable, arguments) in candidates {
            let task = Process()
            task.executableURL = executable
            task.arguments = arguments
            let semaphore = DispatchSemaphore(value: 0)
            task.terminationHandler = { _ in semaphore.signal() }

            do {
                try task.run()
            } catch {
                continue
            }

            let timedOut = semaphore.wait(timeout: .now() + timeout) == .timedOut
            if timedOut {
                task.terminate()
                Thread.sleep(forTimeInterval: 1)
                if task.isRunning {
                    kill(task.processIdentifier, SIGKILL)
                }
                return "Refresh timed out"
            }

            if task.terminationStatus == 0 {
                return nil
            }
            return "Refresh failed (exit \(task.terminationStatus))"
        }

        return "codex-bar-cli not found"
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
