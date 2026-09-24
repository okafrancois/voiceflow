import AppKit
import SwiftUI

/// Palette and metrics taken from the current app (screenshot +
/// `src/index.css`). The native app should be indistinguishable from it.
enum VF {
    // Surfaces
    static let background = dyn(light: 0xF9F9F9, dark: 0x1B1B1B)
    static let card = dyn(light: 0xFFFFFF, dark: 0x212121)
    static let cardHover = dyn(light: 0xF4F4F4, dark: 0x282828)
    static let border = dyn(light: 0xEBEBEB, dark: 0x343434)
    static let divider = dyn(light: 0xF0F0F0, dark: 0x2C2C2C)

    // Text
    static let label = dyn(light: 0x0A0A0A, dark: 0xEDEDED)
    static let labelMuted = dyn(light: 0x737373, dark: 0x8F8F8F)
    static let labelFaint = dyn(light: 0xA3A3A3, dark: 0x6E6E6E)

    // Accents
    static let green = dyn(light: 0x22C55E, dark: 0x4ADE80)
    static let purple = dyn(light: 0xA855F7, dark: 0xC084FC)
    static let amber = dyn(light: 0xF59E0B, dark: 0xFBBF24)
    static let red = dyn(light: 0xEF4444, dark: 0xF87171)

    static let brand = LinearGradient(
        colors: [Color(nsColor: NSColor(hex: 0x35C3E0)), Color(nsColor: NSColor(hex: 0x8B7CF6))],
        startPoint: .topLeading, endPoint: .bottomTrailing)

    // Metrics
    static let sidebarWidth: CGFloat = 248
    static let contentPadding: CGFloat = 40
    static let cardRadius: CGFloat = 18
    static let bandRadius: CGFloat = 24
    static let contentMaxWidth: CGFloat = 1000

    private static func dyn(light: Int, dark: Int) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            NSColor(hex: appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light)
        })
    }
}

extension NSColor {
    convenience init(hex: Int) {
        self.init(
            srgbRed: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            alpha: 1)
    }
}

// MARK: - Building blocks

/// Bordered container: the app's dominant visual motif.
struct VFCard<Content: View>: View {
    var padding: CGFloat = 20
    @ViewBuilder var content: Content

    var body: some View {
        content
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(VF.card, in: RoundedRectangle(cornerRadius: VF.cardRadius, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: VF.cardRadius, style: .continuous)
                    .strokeBorder(VF.border, lineWidth: 1))
    }
}

/// Page title: large title + gray subtitle.
struct VFPageHeader: View {
    let title: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L.t(title))
                .font(.system(size: 28, weight: .semibold))
                .foregroundStyle(VF.label)
            Text(L.t(subtitle))
                .font(.system(size: 14))
                .foregroundStyle(VF.labelMuted)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// Section header: label on the left, underlined link on the right.
struct VFSectionHeader: View {
    let title: String
    var linkTitle: String?
    var action: (() -> Void)?

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(L.t(title))
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(VF.label)
            Spacer()
            if let linkTitle, let action {
                Button(action: action) {
                    Text(linkTitle)
                        .font(.system(size: 13))
                        .foregroundStyle(VF.labelMuted)
                        .underline()
                }
                .buttonStyle(.plain)
            }
        }
    }
}

/// Metric card: label above, large number below.
struct VFMetric: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(L.t(label))
                .font(.system(size: 13))
                .foregroundStyle(VF.labelMuted)
            Text(value)
                .font(.system(size: 36, weight: .regular))
                .foregroundStyle(VF.label)
                .monospacedDigit()
                .lineLimit(1)
                .minimumScaleFactor(0.6)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 20)
        .padding(.vertical, 18)
        .background(VF.card, in: RoundedRectangle(cornerRadius: VF.cardRadius, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: VF.cardRadius, style: .continuous)
                .strokeBorder(VF.border, lineWidth: 1))
    }
}

/// Key shown as a capsule, like the "Fn" badge in the status band.
struct VFKeycap: View {
    let text: String

    var body: some View {
        Text(L.t(text))
            .font(.system(size: 13))
            .foregroundStyle(VF.label)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                Capsule().strokeBorder(VF.border, lineWidth: 1))
    }
}

/// Filled button, fully rounded corners.
struct VFButtonStyle: ButtonStyle {
    var prominent = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(prominent ? Color(nsColor: .windowBackgroundColor) : VF.label)
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .background {
                if prominent {
                    Capsule().fill(VF.label)
                } else {
                    Capsule().strokeBorder(VF.border, lineWidth: 1)
                }
            }
            .opacity(configuration.isPressed ? 0.7 : 1)
    }
}

/// App icon.
struct VFLogo: View {
    var size: CGFloat = 36

    var body: some View {
        RoundedRectangle(cornerRadius: size * 0.26, style: .continuous)
            .fill(VF.brand)
            .frame(width: size, height: size)
            .overlay(
                Image(systemName: "waveform")
                    .font(.system(size: size * 0.48, weight: .bold))
                    .foregroundStyle(.white))
    }
}
