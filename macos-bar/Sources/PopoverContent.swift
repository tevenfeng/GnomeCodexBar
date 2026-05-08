import SwiftUI

/// Main popover content: title + provider cards + refresh button
struct PopoverContent: View {
    @ObservedObject var reader: StatusReader
    @Environment(\.colorScheme) private var colorScheme

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
            }
            .padding(.bottom, 8)

            // ── Provider cards ───────────────────
            if let status = reader.status, !status.providers.isEmpty {
                let sel = reader.selectedProvider
                ForEach(Array(status.providers.enumerated()), id: \.element.id) { index, provider in
                    if index > 0 {
                        // Divider between providers
                        Rectangle()
                            .fill(colorScheme == .dark ? Color.white.opacity(0.08) : Color.black.opacity(0.08))
                            .frame(height: 1)
                            .padding(.vertical, 4)
                    }
                    ProviderCardView(
                        provider: provider,
                        isSelected: provider.providerId == sel,
                        updatedAt: status.updatedAt,
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
        .padding(12)
        .frame(width: 310)
    }

    private func refresh() {
        DispatchQueue.global(qos: .userInitiated).async {
            // Run codex-bar-cli fetch
            let task = Process()
            task.executableURL = URL(fileURLWithPath: "/usr/local/bin/codex-bar-cli")
            task.arguments = ["fetch"]
            try? task.run()
            task.waitUntilExit()

            // Re-read after 2 seconds
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                reader.readStatus()
            }
        }
    }
}
