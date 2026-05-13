import SwiftUI

/// A single progress bar section (title + bar + info row)
struct BarSection: View {
    let title: String
    let percent: Int
    let resetTime: String?
    @Environment(\.colorScheme) private var colorScheme

    private var barColor: Color {
        if percent >= 50 {
            return Color(red: 0.30, green: 0.69, blue: 0.31) // green
        }
        if percent >= 20 {
            return Color(red: 1.0, green: 0.76, blue: 0.03) // yellow
        }
        return Color(red: 0.96, green: 0.26, blue: 0.21) // red
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            // Title
            Text(title)
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(colorScheme == .dark ? .white : Color(red: 0.1, green: 0.1, blue: 0.1))

            // Progress bar
            GeometryReader { geo in
                let barWidth = geo.size.width
                ZStack(alignment: .leading) {
                    // Track
                    RoundedRectangle(cornerRadius: 4)
                        .fill(colorScheme == .dark
                              ? Color.white.opacity(0.12)
                              : Color(red: 0.88, green: 0.88, blue: 0.88))  // #E0E0E0
                        .frame(height: 8)
                    // Fill
                    RoundedRectangle(cornerRadius: 4)
                        .fill(barColor)
                        .frame(width: max(0, CGFloat(percent) / 100.0 * barWidth), height: 8)
                }
            }
            .frame(height: 8)

            // Info row
            HStack {
                Text("\(percent)% left")
                    .font(.system(size: 12))
                    .foregroundColor(colorScheme == .dark
                                     ? Color.white.opacity(0.53)
                                     : Color(red: 0.4, green: 0.4, blue: 0.4))  // #666
                Spacer()
                if let reset = resetTime {
                    Text("Resets in \(reset)")
                        .font(.system(size: 12))
                        .foregroundColor(colorScheme == .dark
                                         ? Color.white.opacity(0.4)
                                         : Color(red: 0.6, green: 0.6, blue: 0.6))  // #999
                }
            }
        }
        .padding(.top, 6)
    }
}
