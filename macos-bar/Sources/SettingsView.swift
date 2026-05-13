import SwiftUI

/// Settings window: update interval + provider enable toggles
struct SettingsView: View {
    @ObservedObject var reader: StatusReader
    @Environment(\.colorScheme) private var colorScheme

    private let intervals: [(String, Double)] = [
        ("5 seconds", 5),
        ("10 seconds", 10),
        ("30 seconds", 30),
        ("1 minute", 60),
        ("5 minutes", 300),
        ("10 minutes", 600),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            // ── Update Interval ──────────────────
            VStack(alignment: .leading, spacing: 8) {
                Text("Update Interval")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundColor(colorScheme == .dark ? .white : Color(red: 0.1, green: 0.1, blue: 0.1))

                Picker("Refresh every", selection: Binding(
                    get: { reader.pollInterval },
                    set: { reader.setPollInterval($0) }
                )) {
                    ForEach(intervals, id: \.1) { label, value in
                        Text(label).tag(value)
                    }
                }
                .pickerStyle(.menu)

                Text("Writes to CLI config.toml and controls daemon refresh interval")
                    .font(.system(size: 11))
                    .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.5) : Color(red: 0.6, green: 0.6, blue: 0.6))
            }

            Divider()

            // ── Providers ────────────────────────
            VStack(alignment: .leading, spacing: 8) {
                Text("Providers")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundColor(colorScheme == .dark ? .white : Color(red: 0.1, green: 0.1, blue: 0.1))

                ForEach(reader.allProviderIDs, id: \.self) { id in
                    let name = reader.providerName(for: id)
                    Toggle(name, isOn: Binding(
                        get: { reader.isProviderEnabled(id) },
                        set: { reader.setProviderEnabled(id, enabled: $0) }
                    ))
                    .font(.system(size: 13))
                    .foregroundColor(colorScheme == .dark ? .white.opacity(0.85) : Color(red: 0.2, green: 0.2, blue: 0.2))
                }

                Text("Disabled providers are written to CLI config.toml")
                    .font(.system(size: 11))
                    .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.5) : Color(red: 0.6, green: 0.6, blue: 0.6))
            }

            Divider()

            // ── Provider Order ───────────────────
            VStack(alignment: .leading, spacing: 8) {
                Text("Provider Order")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundColor(colorScheme == .dark ? .white : Color(red: 0.1, green: 0.1, blue: 0.1))

                ForEach(Array(reader.providerOrder.enumerated()), id: \.element) { index, id in
                    HStack(spacing: 8) {
                        Text(reader.providerName(for: id))
                            .font(.system(size: 13))
                            .foregroundColor(colorScheme == .dark ? .white.opacity(0.85) : Color(red: 0.2, green: 0.2, blue: 0.2))
                        Spacer()
                        Button {
                            moveProvider(from: index, by: -1)
                        } label: {
                            Image(systemName: "chevron.up")
                        }
                        .disabled(index == 0)
                        .buttonStyle(.borderless)

                        Button {
                            moveProvider(from: index, by: 1)
                        } label: {
                            Image(systemName: "chevron.down")
                        }
                        .disabled(index == reader.providerOrder.count - 1)
                        .buttonStyle(.borderless)
                    }
                }

                Text("Controls provider card order in the menu")
                    .font(.system(size: 11))
                    .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.5) : Color(red: 0.6, green: 0.6, blue: 0.6))
            }

            Spacer()
        }
        .padding(24)
        .frame(width: 360, height: 430)
        .background(colorScheme == .dark ? Color(red: 0.12, green: 0.12, blue: 0.12) : Color(red: 0.96, green: 0.96, blue: 0.96))
    }

    private func moveProvider(from index: Int, by delta: Int) {
        let target = index + delta
        guard reader.providerOrder.indices.contains(index), reader.providerOrder.indices.contains(target) else {
            return
        }
        var order = reader.providerOrder
        order.swapAt(index, target)
        reader.setProviderOrder(order)
    }
}
