import Foundation
import Testing
@testable import VoiceFlow

struct TriggerStateMachineTests {
    private let t0 = Date(timeIntervalSinceReferenceDate: 0)
    private func at(_ seconds: TimeInterval) -> Date { t0.addingTimeInterval(seconds) }

    // MARK: Hold

    @Test func holdStartsOnPressAndStopsOnRelease() {
        var machine = TriggerStateMachine(mode: .hold, modifierOnly: false)
        #expect(machine.press(at: at(0)) == .start)
        #expect(machine.release(at: at(2)) == .stop)
    }

    @Test func holdIgnoresTheDuplicateEventSomeKeyboardsSend() {
        var machine = TriggerStateMachine(mode: .hold, modifierOnly: true)
        #expect(machine.press(at: at(0)) == .start)
        #expect(machine.press(at: at(0.02)) == nil)
        #expect(machine.release(at: at(1)) == .stop)
    }

    @Test func modifierUsedInAKeyboardShortcutCancelsTheDictation() {
        // Fn + ↑, right ⌘ + C: the user was typing a shortcut, not dictating.
        var machine = TriggerStateMachine(mode: .hold, modifierOnly: true)
        #expect(machine.press(at: at(0)) == .start)
        #expect(machine.otherKey(at: at(0.2)) == .cancel)
        #expect(machine.release(at: at(0.4)) == nil)
    }

    @Test func aKeyPressedLongIntoADictationDoesNotCancelIt() {
        var machine = TriggerStateMachine(mode: .hold, modifierOnly: true)
        _ = machine.press(at: at(0))
        #expect(machine.otherKey(at: at(5)) == nil)
        #expect(machine.release(at: at(6)) == .stop)
    }

    @Test func otherKeysNeverCancelAComboShortcut() {
        var machine = TriggerStateMachine(mode: .hold, modifierOnly: false)
        _ = machine.press(at: at(0))
        #expect(machine.otherKey(at: at(0.1)) == nil)
    }

    // MARK: Toggle

    @Test func toggleStartsThenStopsOnSuccessivePresses() {
        var machine = TriggerStateMachine(mode: .toggle, modifierOnly: false)
        #expect(machine.press(at: at(0)) == .start)
        #expect(machine.release(at: at(0.1)) == nil)
        #expect(machine.press(at: at(3)) == .stop)
    }

    @Test func modifierOnlyToggleActsOnReleaseAndOnlyWhenUsedAlone() {
        var machine = TriggerStateMachine(mode: .toggle, modifierOnly: true)
        #expect(machine.press(at: at(0)) == nil)
        #expect(machine.release(at: at(0.1)) == .start)

        #expect(machine.press(at: at(2)) == nil)
        #expect(machine.otherKey(at: at(2.1)) == nil)
        #expect(machine.release(at: at(2.2)) == nil)

        #expect(machine.press(at: at(4)) == nil)
        #expect(machine.release(at: at(4.1)) == .stop)
    }

    @Test func toggleResynchronisesWhenTheAppEndsTheDictationItself() {
        var machine = TriggerStateMachine(mode: .toggle, modifierOnly: false)
        #expect(machine.press(at: at(0)) == .start)
        _ = machine.release(at: at(0.1))
        machine.dictationEnded()
        #expect(machine.press(at: at(3)) == .start)
    }

    // MARK: Double tap

    @Test func doubleTapNeedsTwoQuickPresses() {
        var machine = TriggerStateMachine(mode: .doubleTap, modifierOnly: false)
        #expect(machine.press(at: at(0)) == nil)
        _ = machine.release(at: at(0.05))
        #expect(machine.press(at: at(0.3)) == .start)
        _ = machine.release(at: at(0.35))
        #expect(machine.press(at: at(4)) == .stop)
    }

    @Test func doubleTapTooSlowDoesNothing() {
        var machine = TriggerStateMachine(mode: .doubleTap, modifierOnly: false)
        _ = machine.press(at: at(0))
        _ = machine.release(at: at(0.05))
        #expect(machine.press(at: at(1)) == nil)
    }

    @Test func modifierOnlyDoubleTapCountsCleanTapsOnly() {
        var machine = TriggerStateMachine(mode: .doubleTap, modifierOnly: true)
        _ = machine.press(at: at(0))
        #expect(machine.release(at: at(0.05)) == nil)
        _ = machine.press(at: at(0.2))
        #expect(machine.release(at: at(0.25)) == .start)
    }
}
