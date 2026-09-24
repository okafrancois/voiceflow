import AppKit
import SwiftUI

/// Thème de l'application : suivre le système, ou forcer clair/sombre.
enum AppTheme: String, CaseIterable, Identifiable {
    case system, light, dark
    var id: String { rawValue }

    var title: String {
        switch self {
        case .system: L.t("Système")
        case .light: L.t("Clair")
        case .dark: L.t("Sombre")
        }
    }

    var appearance: NSAppearance? {
        switch self {
        case .system: nil
        case .light: NSAppearance(named: .aqua)
        case .dark: NSAppearance(named: .darkAqua)
        }
    }

    @MainActor
    func apply() {
        NSApp.appearance = appearance
    }
}

/// Ancrage de la pill sur l'écran. Le glisser-déposer reste possible ; choisir
/// un ancrage y ramène la pill.
enum PillPosition: String, CaseIterable, Identifiable {
    case topLeft, topCenter, topRight, bottomLeft, bottomCenter, bottomRight, free
    var id: String { rawValue }

    var title: String {
        switch self {
        case .topLeft: L.t("Haut gauche")
        case .topCenter: L.t("Haut centre")
        case .topRight: L.t("Haut droite")
        case .bottomLeft: L.t("Bas gauche")
        case .bottomCenter: L.t("Bas centre")
        case .bottomRight: L.t("Bas droite")
        case .free: L.t("Libre (déplacée à la main)")
        }
    }

    /// Centre de la pill pour cet ancrage, dans le cadre visible de l'écran.
    func center(in frame: NSRect, size: NSSize) -> NSPoint {
        let margin: CGFloat = 28
        let x: CGFloat = switch self {
        case .topLeft, .bottomLeft: frame.minX + size.width / 2 + margin
        case .topRight, .bottomRight: frame.maxX - size.width / 2 - margin
        default: frame.midX
        }
        let y: CGFloat = switch self {
        case .topLeft, .topCenter, .topRight: frame.maxY - size.height / 2 - margin
        default: frame.minY + size.height / 2 + margin
        }
        return NSPoint(x: x, y: y)
    }
}

/// Quand la pill doit-elle être visible ?
enum PillVisibility: String, CaseIterable, Identifiable {
    case always, whileActive, never
    var id: String { rawValue }

    var title: String {
        switch self {
        case .always: L.t("Toujours")
        case .whileActive: L.t("Pendant la dictée")
        case .never: L.t("Jamais")
        }
    }
}

/// Teintes de fond proposées, reprises de l'app actuelle.
enum PillTint: String, CaseIterable, Identifiable {
    case dark, slate, forest, plum, copper
    var id: String { rawValue }

    var title: String {
        switch self {
        case .dark: L.t("Sombre")
        case .slate: L.t("Bleu ardoise")
        case .forest: L.t("Vert forêt")
        case .plum: L.t("Prune")
        case .copper: L.t("Cuivre")
        }
    }

    var hex: Int {
        switch self {
        case .dark: 0x1D1D1D
        case .slate: 0x26324A
        case .forest: 0x2F3A32
        case .plum: 0x472B39
        case .copper: 0x4A3728
        }
    }

    var color: Color { Color(nsColor: NSColor(hex: hex)) }
}
