import Testing
@testable import VoiceFlow

/// Some fields (Chromium/Electron) report the AX attribute as settable and
/// return success without inserting anything: the write must be checked.
struct InsertionVerdictTests {
    @Test func changedValueMeansTheTextLanded() {
        #expect(AccessibilityTarget.verdict(before: "Hello", after: "Hello world") == .landed)
    }

    @Test func unchangedValueMeansTheFieldIgnoredTheWrite() {
        #expect(AccessibilityTarget.verdict(before: "Hello", after: "Hello") == .ignored)
    }

    @Test func emptyFieldLeftEmptyIsIgnored() {
        #expect(AccessibilityTarget.verdict(before: "", after: "") == .ignored)
    }

    @Test func unreadableValueCannotBeJudged() {
        #expect(AccessibilityTarget.verdict(before: nil, after: "Hello") == .unknown)
        #expect(AccessibilityTarget.verdict(before: "Hello", after: nil) == .unknown)
    }
}
