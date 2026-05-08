import SwiftUI

/// A single provider card (header + progress bars + details)
struct ProviderCardView: View {
    let provider: ProviderStatus
    let isSelected: Bool
    let updatedAt: String?
    let onSelect: () -> Void

    @ObservedObject var reader: StatusReader
    @Environment(\.colorScheme) private var colorScheme

    private var d: [String: JSONValue] { provider.details }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // ── Header (clickable) ──────────────────
            Button(action: onSelect) {
                VStack(alignment: .leading, spacing: 2) {
                    // Row 1: Name + Plan tag
                    HStack {
                        Text(provider.providerName)
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundColor(isSelected
                                             ? (colorScheme == .dark ? Color(red: 0.21, green: 0.52, blue: 0.89) : Color(red: 0.1, green: 0.43, blue: 0.83))
                                             : (colorScheme == .dark ? Color.white.opacity(0.8) : Color(red: 0.2, green: 0.2, blue: 0.2)))
                        Spacer()
                        // Plan tag (StepFun only)
                        if provider.providerId == "stepfun", let plan = d["plan_name"]?.stringValue {
                            Text(plan)
                                .font(.system(size: 12))
                                .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.6) : Color(red: 0.4, green: 0.4, blue: 0.4))
                        }
                    }
                    // Row 2: Updated time
                    Text("Updated \(reader.ago(updatedAt))")
                        .font(.system(size: 12))
                        .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.4) : Color(red: 0.6, green: 0.6, blue: 0.6))
                }
                .buttonStyle(.plain)
                .contentShape(Rectangle())
            }

            // Divider
            Rectangle()
                .fill(colorScheme == .dark ? Color.white.opacity(0.08) : Color.black.opacity(0.08))
                .frame(height: 1)
                .padding(.vertical, 8)

            // ── Details ─────────────────────────────
            if provider.error != nil {
                Text("Error: \(provider.error ?? "Unknown")")
                    .font(.system(size: 12))
                    .foregroundColor(Color(red: 0.8, green: 0.2, blue: 0.2))
            } else {
                detailsContent
            }
        }
        .padding(10)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected
                      ? (colorScheme == .dark
                         ? Color(red: 0.21, green: 0.52, blue: 0.89).opacity(0.12)
                         : Color(red: 0.1, green: 0.43, blue: 0.83).opacity(0.08))
                      : (colorScheme == .dark
                         ? Color.white.opacity(0.05)
                         : Color.black.opacity(0.04)))
        )
    }

    @ViewBuilder
    private var detailsContent: some View {
        if provider.providerId == "deepseek" {
            deepSeekDetails
        } else {
            stepFunDetails
        }
    }

    // ── DeepSeek ──────────────────────────────────────

    private var deepSeekDetails: some View {
        let total = d["total_balance"]?.doubleValue ?? 0
        let hasBalance = total > 0
        let pct = hasBalance ? 100 : 0

        return VStack(alignment: .leading, spacing: 0) {
            BarSection(title: "Balance", percent: pct, resetTime: nil)

            // Balance detail text
            let cu = d["currency"]?.stringValue ?? "CNY"
            let sym = cu == "USD" ? "$" : "¥"
            let paid = d["topped_up_balance"]?.doubleValue ?? 0
            let granted = d["granted_balance"]?.doubleValue ?? 0

            Text("\(sym)\(String(format: "%.2f", total)) (Paid: \(sym)\(String(format: "%.2f", paid)) / Granted: \(sym)\(String(format: "%.2f", granted)))")
                .font(.system(size: 11))
                .foregroundColor(colorScheme == .dark ? Color.white.opacity(0.6) : Color(red: 0.4, green: 0.4, blue: 0.4))
                .padding(.top, 4)
        }
    }

    // ── StepFun ───────────────────────────────────────

    private var stepFunDetails: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let rate = d["five_hour_usage_left_rate"]?.doubleValue {
                let pct = Int((rate * 100).rounded())
                let reset = d["five_hour_usage_reset_time"]?.stringValue
                BarSection(title: "5h Window", percent: pct, resetTime: reader.resetIn(reset))
            }
            if let rate = d["weekly_usage_left_rate"]?.doubleValue {
                let pct = Int((rate * 100).rounded())
                let reset = d["weekly_usage_reset_time"]?.stringValue
                BarSection(title: "Weekly Window", percent: pct, resetTime: reader.resetIn(reset))
            }
        }
    }
}
