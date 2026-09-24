import Foundation

/// Translates shortcut presses into dictation commands, based on the mode.
///
/// No system state: `HotkeyManager` passes it the events, clock
/// included, which makes it possible to test each mode identically.
///
/// A shortcut using a standalone modifier key (Fn, right ⌘…) is also
/// used in ordinary combinations (Fn + ↑, ⌘ + C). In hold mode,
/// dictation starts on press for immediate feedback, and cancels if
/// another key follows shortly after; in the other modes, action only
/// happens on release, and only if the key was pressed alone.
struct TriggerStateMachine {
    enum Action: Equatable {
        case start, stop, cancel
    }

    var mode: TriggerMode
    var modifierOnly: Bool

    /// Another key pressed within this delay after the start: it was a
    /// combination, not a dictation.
    static let chordWindow: TimeInterval = 1.0
    static let doubleTapWindow: TimeInterval = 0.4
    /// Some keyboards emit two events for a single press.
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

    /// A key other than the shortcut was pressed.
    mutating func otherKey(at now: Date) -> Action? {
        guard isHeld, modifierOnly else { return nil }
        chorded = true
        guard mode == .hold, isActive, let pressedAt,
              now.timeIntervalSince(pressedAt) < Self.chordWindow
        else { return nil }
        isActive = false
        return .cancel
    }

    /// The app went back to idle on its own (cancellation, error, end of
    /// processing): what the machine believed no longer holds.
    mutating func dictationEnded() {
        isActive = false
        lastTapAt = nil
    }

    /// Settings changed: start over from scratch.
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
