import SwiftUI

/// A single progress bar section (title + bar + info row)
struct BarSection: View {
    let title: String
    let percent: Int
    let resetTime: String?
    @Environment(\.colorScheme) private var colorScheme

    private var barColor: Color {
        if percent < 20 { return Color(red: 0.8, green: 0.2, blue: 0.2) }      // err
        if percent < 50 { return Color(red: 0.9, green: 0.66, blue: 0.09) }     // warn
        return Color(red: 0.2, green: 0.8, blue: 0.4)                            // ok
    }

    private var barColorLight: Color {
        if percent < 20 { return Color(red: 0.8, green: 0.2, blue: 0.2) }
        if percent < 50 { return Color(red: 0.77, green: 0.54, blue: 0) }
        return Color(red: 0.18, green: 0.63, blue: 0.26)                         // ok (light)
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
                              : Color.black.opacity(0.1))
                        .frame(height: 8)
                    // Fill
                    RoundedRectangle(cornerRadius: 4)
                        .fill(colorScheme == .dark ? barColor : barColorLight)
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
                                     : Color(red: 0.33, green: 0.33, blue: 0.33))
                Spacer()
                if let reset = resetTime {
                    Text("Resets in \(reset)")
                        .font(.system(size: 12))
                        .foregroundColor(colorScheme == .dark
                                         ? Color.white.opacity(0.4)
                                         : Color(red: 0.6, green: 0.6, blue: 0.6))
                }
            }
        }
        .padding(.top, 10)
    }
}
