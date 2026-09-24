import AppKit
import Carbon.HIToolbox
import Testing
@testable import VoiceFlow

struct ShortcutTests {
    private let rightCommand: UInt16 = 54
    private let leftCommandBits: UInt = 0x08
    private let rightCommandBits: UInt = 0x10

    @Test func comboMatchesOnlyItsExactModifiers() {
        let shortcut = Shortcut.dictateDefault // ⌥ Space
        #expect(shortcut.matches(keyCode: UInt16(kVK_Space), flags: .option))
        #expect(!shortcut.matches(keyCode: UInt16(kVK_Space), flags: [.option, .shift]))
        #expect(!shortcut.matches(keyCode: UInt16(kVK_Space), flags: []))
    }

    @Test func modifierOnlyShortcutNeverMatchesOrdinaryKeystrokes() {
        let fn = Shortcut(keyCode: 63, modifiers: 0)
        #expect(fn.isModifierOnly)
        #expect(!fn.matches(keyCode: 63, flags: .function))
    }

    @Test func rightCommandIsReleasedEvenWhileLeftCommandIsHeld() {
        let leftHeld = NSEvent.ModifierFlags(rawValue: NSEvent.ModifierFlags.command.rawValue | leftCommandBits)
        #expect(!Shortcut.isPressed(rightCommand, leftHeld))
        let rightHeld = NSEvent.ModifierFlags(rawValue: NSEvent.ModifierFlags.command.rawValue | rightCommandBits)
        #expect(Shortcut.isPressed(rightCommand, rightHeld))
    }

    @Test func fnHasNoSideAndUsesTheGenericFlag() {
        #expect(Shortcut.isPressed(63, .function))
        #expect(!Shortcut.isPressed(63, []))
    }

    @Test func plainLettersAreNotAssignableAlone() {
        #expect(!Shortcut.isAssignable(keyCode: UInt16(kVK_ANSI_J), modifiers: []))
        #expect(Shortcut.isAssignable(keyCode: UInt16(kVK_ANSI_J), modifiers: .option))
        #expect(Shortcut.isAssignable(keyCode: UInt16(kVK_F5), modifiers: []))
    }
}
