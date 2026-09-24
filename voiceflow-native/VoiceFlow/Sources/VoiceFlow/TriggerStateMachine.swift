import Foundation

/// Traduit les pressions du raccourci en ordres de dictée, selon le mode.
///
/// Sans état système : `HotkeyManager` lui passe les événements, l'horloge
/// comprise, ce qui permet de tester chaque mode à l'identique.
///
/// Un raccourci à touche modificatrice seule (Fn, ⌘ droite…) sert aussi dans
/// des combinaisons ordinaires (Fn + ↑, ⌘ + C). En mode maintenir, la dictée
/// démarre à la pression pour un retour immédiat, et s'annule si une autre
/// touche suit de près ; dans les autres modes, on n'agit qu'au relâchement,
/// et seulement si la touche a été pressée seule.
struct TriggerStateMachine {
    enum Action: Equatable {
        case start, stop, cancel
    }

    var mode: TriggerMode
    var modifierOnly: Bool

    /// Une autre touche pressée dans ce délai après le début : c'était une
    /// combinaison, pas une dictée.
    static let chordWindow: TimeInterval = 1.0
    static let doubleTapWindow: TimeInterval = 0.4
    /// Certains claviers émettent deux événements pour une seule pression.
    static let bounceWindow: TimeInterval = 0.08

    private(set) var isHeld = false
    private(set) var isActive = false
    private var pressedAt: Date?
    private var lastPressAt: Date?
    private var lastTapAt: Date?
    private var chorded = false

    init(mode: TriggerMode, modifierOnly: Bool) {
        self.mode = mode
        self.modifierOnly = modifierOnly
    }

    mutating func press(at now: Date) -> Action? {
        if let lastPressAt, now.timeIntervalSince(lastPressAt) < Self.bounceWindow {
            return nil
        }
        lastPressAt = now
        guard !isHeld else { return nil }
        isHeld = true
        pressedAt = now
        chorded = false

        switch mode {
        case .hold:
            guard !isActive else { return nil }
            isActive = true
            return .start
        case .toggle:
            return modifierOnly ? nil : toggle()
        case .doubleTap:
            return modifierOnly ? nil : tap(at: now)
        }
    }

    mutating func release(at now: Date) -> Action? {
        guard isHeld else { return nil }
        isHeld = false
        switch mode {
        case .hold:
            guard isActive else { return nil }
            isActive = false
            return .stop
        case .toggle:
            return modifierOnly && !chorded ? toggle() : nil
        case .doubleTap:
            return modifierOnly && !chorded ? tap(at: now) : nil
        }
    }

    /// Une touche autre que le raccourci a été pressée.
    mutating func otherKey(at now: Date) -> Action? {
        guard isHeld, modifierOnly else { return nil }
        chorded = true
        guard mode == .hold, isActive, let pressedAt,
              now.timeIntervalSince(pressedAt) < Self.chordWindow
        else { return nil }
        isActive = false
        return .cancel
    }

    /// L'app est revenue au repos d'elle-même (annulation, erreur, fin du
    /// traitement) : ce que croyait la machine n'a plus cours.
    mutating func dictationEnded() {
        isActive = false
        lastTapAt = nil
    }

    /// Réglages modifiés : repartir de zéro.
    mutating func reset() {
        isHeld = false
        isActive = false
        pressedAt = nil
        lastPressAt = nil
        lastTapAt = nil
        chorded = false
    }

    private mutating func toggle() -> Action {
        isActive.toggle()
        return isActive ? .start : .stop
    }

    private mutating func tap(at now: Date) -> Action? {
        if isActive {
            isActive = false
            lastTapAt = nil
            return .stop
        }
        if let lastTapAt, now.timeIntervalSince(lastTapAt) < Self.doubleTapWindow {
            self.lastTapAt = nil
            isActive = true
            return .start
        }
        lastTapAt = now
        return nil
    }
}
