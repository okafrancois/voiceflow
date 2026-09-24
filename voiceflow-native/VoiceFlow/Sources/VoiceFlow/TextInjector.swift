import AppKit
import ApplicationServices
import CoreGraphics

/// Swift port of `apps/desktop/src-tauri/src/text_injector/` (Rust), which is
/// the reference for behavior:
/// - AX target captured at dictation start → insertion without reactivating the app;
/// - otherwise: multiline or > 400 graphemes → clipboard + Cmd+V (with full
///   clipboard save/restore); short text → simulated keyboard typing in
///   100-grapheme chunks spaced 50 ms apart.

enum InjectionMethod {
    case keyboard, clipboard, accessibility
}

enum InjectionError: LocalizedError {
    case noCapturedTarget
    case attributeNotWritable(String)
    case axRejected(String, AXError)
    case writeIgnored
    case keyboardAndClipboardFailed
    case clipboardPasteFailed

    var errorDescription: String? {
        switch self {
        case .noCapturedTarget:
            L.t("Le champ d'origine n'a pas été capturé")
        case .attributeNotWritable(let attribute):
            L.t("Le champ d'origine n'est pas modifiable") + " (\(attribute))"
        case .axRejected(let attribute, let code):
            L.t("Le champ d'origine a refusé l'insertion") + " (\(attribute), \(code.rawValue))"
        case .writeIgnored:
            L.t("Le champ d'origine a accepté l'écriture sans rien insérer")
        case .keyboardAndClipboardFailed:
            L.t("Injection clavier et presse-papiers échouées")
        case .clipboardPasteFailed:
            L.t("Collage via le presse-papiers échoué")
        }
    }
}

// MARK: - Accessibility queue

/// All accessibility exchanges with another app go through here.
///
/// Each call waits for the queried app's response; on the main thread, a
/// slow app would freeze the interface — and, before the shortcut had its
/// own thread, the entire keyboard. A serial queue also preserves order:
/// capture, insertion, readback.
enum AXQueue {
    private static let queue = DispatchQueue(
        label: "fr.okatech.voiceflow.accessibility", qos: .userInitiated)

    static func run<T: Sendable>(_ work: @escaping @Sendable () -> T) async -> T {
        await withCheckedContinuation { continuation in
            queue.async { continuation.resume(returning: work()) }
        }
    }

    static func run<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async { continuation.resume(with: Result { try work() }) }
        }
    }
}

// MARK: - Accessibility target

/// Snapshot of the field that has focus when recording starts: AX element +
/// cursor position. Allows inserting later without reactivating the
/// application.
/// `AXUIElement` is a Core Foundation reference usable from any thread;
/// the exchanges themselves go through `AXQueue`.
struct AccessibilityTarget: @unchecked Sendable {
    private let element: AXUIElement
    private let selectedRange: CFRange?

    /// Maximum delay granted to the queried app. By default macOS waits
    /// six seconds for an app that doesn't respond.
    static let messagingTimeout: Float = 1.0

    static func capture() -> AccessibilityTarget? {
        let system = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(system, messagingTimeout)
        var focusedValue: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(
            system, kAXFocusedUIElementAttribute as CFString, &focusedValue)
        guard error == .success, let focusedValue else { return nil }
        let element = focusedValue as! AXUIElement
        AXUIElementSetMessagingTimeout(element, messagingTimeout)

        guard isSettable(element, kAXSelectedTextAttribute) else { return nil }

        var selectedRange: CFRange?
        if isSettable(element, kAXSelectedTextRangeAttribute) {
            selectedRange = copyRange(element, kAXSelectedTextRangeAttribute)
        }
        return AccessibilityTarget(element: element, selectedRange: selectedRange)
    }

    /// Re-reads the field's content, to spot a correction after the fact.
    func readValue() -> String? {
        var value: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(
            element, kAXValueAttribute as CFString, &value)
        guard error == .success else { return nil }
        return value as? String
    }

    /// Inserts at the captured position and returns that position (in
    /// UTF-16 units, those used by accessibility), if it is known.
    @discardableResult
    func insert(_ text: String) throws -> Int? {
        let before = readValue()
        let location = selectedRange?.location
            ?? Self.copyRange(element, kAXSelectedTextRangeAttribute)?.location
        if let selectedRange {
            try setRange(element, kAXSelectedTextRangeAttribute, selectedRange)
        }
        var timedOut = false
        do {
            try setString(element, kAXSelectedTextAttribute, text)
        } catch InjectionError.axRejected(_, .cannotComplete) {
            // Timeout exceeded: the app may have written anyway. Verify before
            // falling back, otherwise the text would arrive twice.
            log.warning("accessibility write timed out, verifying")
            timedOut = true
        }

        // Chromium/Electron declare the attribute writable and return
        // .success without inserting anything: re-read the field to make sure.
        // Writing there is asynchronous, hence a few spaced-out re-reads.
        for attempt in 0..<Self.verifyAttempts {
            switch Self.verdict(before: before, after: readValue()) {
            case .landed: return location
            // Without a timeout, accessibility said yes: trust it.
            // After a timeout and an unreadable field, nothing proves the
            // write happened: better to fall back than lose a dictation.
            case .unknown: if !timedOut { return location }
            case .ignored: break
            }
            if attempt < Self.verifyAttempts - 1 {
                usleep(Self.verifyDelayUs)
            }
        }
        throw InjectionError.writeIgnored
    }

    /// Removes text inserted earlier, provided it is still intact in
    /// place: never a blind erase. `restoring` takes its place (the
    /// selection that the insertion had replaced).
    func remove(_ text: String, at location: Int, restoring: String = "") -> Bool {
        guard let value = readValue() as NSString? else { return false }
        let length = (text as NSString).length
        guard location >= 0, location + length <= value.length,
              value.substring(with: NSRange(location: location, length: length)) == text
        else { return false }
        do {
            try setRange(element, kAXSelectedTextRangeAttribute,
                         CFRange(location: location, length: length))
            try setString(element, kAXSelectedTextAttribute, restoring)
            return true
        } catch {
            log.error("undo via accessibility failed: \(error.localizedDescription)")
            return false
        }
    }

    /// The text preceding the insertion position, to decide on
    /// spacing and capitalization. `nil` if the field doesn't say.
    func textBeforeInsertion(maxLength: Int = 64) -> String? {
        guard let value = readValue() as NSString? else { return nil }
        let location = selectedRange?.location
            ?? Self.copyRange(element, kAXSelectedTextRangeAttribute)?.location
        guard let location, location >= 0, location <= value.length else { return nil }
        let start = max(0, location - maxLength)
        return value.substring(with: NSRange(location: start, length: location - start))
    }

    /// The text selected at the moment of capture (command mode).
    func selectedText() -> String? {
        var value: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(
            element, kAXSelectedTextAttribute as CFString, &value)
        guard error == .success, let text = value as? String, !text.isEmpty else { return nil }
        return text
    }

    enum Verdict {
        case landed, ignored, unknown
    }

    private static let verifyAttempts = 6
    private static let verifyDelayUs: UInt32 = 50_000

    /// Content unchanged after writing: the field ignored it. Unreadable
    /// content: impossible to decide, so we trust the AX success.
    static func verdict(before: String?, after: String?) -> Verdict {
        guard let before, let after else { return .unknown }
        return before == after ? .ignored : .landed
    }

    // Helpers AX

    private static func isSettable(_ element: AXUIElement, _ attribute: String) -> Bool {
        var settable = DarwinBoolean(false)
        let error = AXUIElementIsAttributeSettable(element, attribute as CFString, &settable)
        return error == .success && settable.boolValue
    }

    private static func copyRange(_ element: AXUIElement, _ attribute: String) -> CFRange? {
        var value: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(element, attribute as CFString, &value)
        guard error == .success, let value else { return nil }
        var range = CFRange(location: 0, length: 0)
        guard AXValueGetValue(value as! AXValue, .cfRange, &range) else { return nil }
        return range
    }

    private func setRange(_ element: AXUIElement, _ attribute: String, _ range: CFRange) throws {
        guard Self.isSettable(element, attribute) else {
            throw InjectionError.attributeNotWritable(attribute)
        }
        var range = range
        guard let value = AXValueCreate(.cfRange, &range) else {
            throw InjectionError.attributeNotWritable(attribute)
        }
        let error = AXUIElementSetAttributeValue(element, attribute as CFString, value)
        guard error == .success else {
            throw InjectionError.axRejected(attribute, error)
        }
    }

    private func setString(_ element: AXUIElement, _ attribute: String, _ text: String) throws {
        guard Self.isSettable(element, attribute) else {
            throw InjectionError.attributeNotWritable(attribute)
        }
        let error = AXUIElementSetAttributeValue(element, attribute as CFString, text as CFString)
        guard error == .success else {
            throw InjectionError.axRejected(attribute, error)
        }
    }
}

/// Target captured at the moment dictation starts: the editable field and
/// the frontmost application (which determines the polish style).
final class CapturedTextTarget: Sendable {
    let accessibility: AccessibilityTarget?
    let bundleID: String?
    let appName: String?
    /// Text selected at trigger time (command mode).
    let selection: String?

    init(accessibility: AccessibilityTarget?, bundleID: String?, appName: String?,
         selection: String? = nil) {
        self.accessibility = accessibility
        self.bundleID = bundleID
        self.appName = appName
        self.selection = selection
    }

    /// The frontmost app is read right away; the field, however, requires
    /// querying the target app, hence on the accessibility queue.
    static func capture(
        accessibility: Bool, bundleID: String?, appName: String?, readSelection: Bool = true
    ) async -> CapturedTextTarget {
        guard accessibility else {
            return CapturedTextTarget(accessibility: nil, bundleID: bundleID, appName: appName)
        }
        let (element, selection) = await AXQueue.run {
            let element = AccessibilityTarget.capture()
            return (element, readSelection ? element?.selectedText() : nil)
        }
        return CapturedTextTarget(
            accessibility: element, bundleID: bundleID, appName: appName, selection: selection)
    }

}

// MARK: - Injection into current focus

enum TextInjector {
    private static let chunkSize = 100
    private static let chunkDelayMs: UInt64 = 50
    private static let clipboardThreshold = 400

    static func insert(_ text: String) throws -> InjectionMethod {
        waitForModifierRelease()
        let graphemeCount = text.count
        let hasNewline = text.contains("\n")
        log.info("injection started: \(text.utf8.count) bytes, \(graphemeCount) graphemes, newline=\(hasNewline)")

        if hasNewline || graphemeCount > clipboardThreshold {
            guard clipboardPaste(text) else {
                throw InjectionError.clipboardPasteFailed
            }
            return .clipboard
        }

        if typeText(text) {
            return .keyboard
        }
        log.info("keyboard injection failed, falling back to clipboard")
        guard clipboardPaste(text) else {
            throw InjectionError.keyboardAndClipboardFailed
        }
        return .clipboard
    }

    // MARK: Layer 0 — simulated keyboard typing

    /// Simulates typing without touching the clipboard. Chunks of
    /// 100 graphemes with a 50 ms pause to avoid breaking IME composition;
    /// each CGEvent carries at most 20 UTF-16 units.
    private static func typeText(_ text: String) -> Bool {
        guard let source = CGEventSource(stateID: .combinedSessionState) else {
            return false
        }
        let characters = Array(text)
        let chunks = stride(from: 0, to: characters.count, by: chunkSize).map {
            String(characters[$0..<min($0 + chunkSize, characters.count)])
        }
        for (index, chunk) in chunks.enumerated() {
            guard postUnicodeString(chunk, source: source) else { return false }
            if index < chunks.count - 1 {
                usleep(UInt32(chunkDelayMs) * 1000)
            }
        }
        return true
    }

    private static func postUnicodeString(_ text: String, source: CGEventSource) -> Bool {
        let units = Array(text.utf16)
        var start = 0
        while start < units.count {
            let slice = Array(units[start..<min(start + 20, units.count)])
            guard
                let down = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true),
                let up = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: false)
            else { return false }
            // Without explicit flags, the event inherits keys still held
            // down (the shortcut's ⌥) and the app may read it as a shortcut.
            down.flags = []
            up.flags = []
            down.keyboardSetUnicodeString(stringLength: slice.count, unicodeString: slice)
            up.keyboardSetUnicodeString(stringLength: slice.count, unicodeString: slice)
            down.post(tap: .cghidEventTap)
            up.post(tap: .cghidEventTap)
            start += 20
        }
        return true
    }

    // MARK: Layer 2 — clipboard + Cmd+V

    /// Delay before restoring the clipboard. A busy Electron app reads
    /// the content well after ⌘V; restored too early (100 ms
    /// previously), it would paste the old content.
    private static let restoreDelay: DispatchTimeInterval = .milliseconds(700)

    /// Types recognized by clipboard managers
    /// (nspasteboard.org): ephemeral content, not to be archived.
    private static let transientTypes: [NSPasteboard.PasteboardType] = [
        NSPasteboard.PasteboardType("org.nspasteboard.TransientType"),
        NSPasteboard.PasteboardType("org.nspasteboard.ConcealedType"),
        NSPasteboard.PasteboardType("org.nspasteboard.AutoGeneratedType"),
    ]

    /// Pending restore: a second paste before it happens must pick up
    /// the original content, not our own text.
    /// Read and written only on the main thread.
    @MainActor private static var pendingRestore: (snapshot: PasteboardSnapshot?, change: Int)?
    @MainActor private static var restoreGeneration = 0

    private static func clipboardPaste(_ text: String) -> Bool {
        let (saved, generation): (PasteboardSnapshot?, Int) = DispatchQueue.main.sync {
            MainActor.assumeIsolated {
                let pasteboard = NSPasteboard.general
                let snapshot: PasteboardSnapshot?
                if let pending = pendingRestore, pending.change == pasteboard.changeCount {
                    snapshot = pending.snapshot
                } else {
                    snapshot = PasteboardSnapshot.capture()
                }
                restoreGeneration += 1
                return (snapshot, restoreGeneration)
            }
        }
        let ourChange: Int = DispatchQueue.main.sync {
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            let item = NSPasteboardItem()
            item.setString(text, forType: .string)
            for type in transientTypes { item.setData(Data(), forType: type) }
            pasteboard.writeObjects([item])
            let change = pasteboard.changeCount
            MainActor.assumeIsolated { pendingRestore = (saved, change) }
            return change
        }

        let ok = pressKey(0x09, flags: .maskCommand)

        DispatchQueue.main.asyncAfter(deadline: .now() + restoreDelay) {
            // A more recent paste will take care of the restore.
            let isLatest = MainActor.assumeIsolated {
                guard restoreGeneration == generation else { return false }
                pendingRestore = nil
                return true
            }
            guard isLatest else { return }
            // Someone copied something in the meantime: their content takes priority over ours.
            guard NSPasteboard.general.changeCount == ourChange else {
                log.info("clipboard changed meanwhile, not restoring")
                return
            }
            if let saved {
                saved.restore()
                log.info("clipboard restored")
            } else {
                NSPasteboard.general.clearContents()
                log.info("clipboard cleared")
            }
        }
        return ok
    }

    /// ⌘Z in the frontmost app.
    static func pressUndo() {
        waitForModifierRelease()
        // 0x06 = physical 'z'.
        _ = pressKey(0x06, flags: .maskCommand)
    }

    /// 0x09 = 'v', 0x06 = 'z': physical keys, independent of the
    /// keyboard layout.
    private static func pressKey(_ key: CGKeyCode, flags: CGEventFlags) -> Bool {
        guard let source = CGEventSource(stateID: .combinedSessionState) else {
            return false
        }
        // Let the target app regain focus before receiving the keys.
        usleep(20_000)
        guard
            let down = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: true),
            let up = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: false)
        else { return false }
        down.flags = flags
        up.flags = flags
        down.post(tap: .cghidEventTap)
        up.post(tap: .cghidEventTap)
        return true
    }

    /// Waits for the user to release ⌘ ⌥ ⌃ ⇧ (at most one second):
    /// a simulated keystroke while the shortcut's ⌥ is still held
    /// becomes a special character or an app shortcut.
    static func waitForModifierRelease(timeout: TimeInterval = 1.0) {
        let held: CGEventFlags = [.maskCommand, .maskAlternate, .maskControl, .maskShift]
        let deadline = Date().addingTimeInterval(timeout)
        while !CGEventSource.flagsState(.combinedSessionState).intersection(held).isEmpty,
              Date() < deadline {
            usleep(10_000)
        }
    }
}

// MARK: - Clipboard save/restore

/// Full copy of the clipboard (all items, all types), restored
/// after the paste so the user loses nothing.
///
/// Created and restored on the main thread only; it merely passes
/// through the insertion thread.
private struct PasteboardSnapshot: @unchecked Sendable {
    private enum Value {
        case plist(Any)
        case data(Data)
        case string(String)
    }
    private struct Item {
        var types: [(NSPasteboard.PasteboardType, Value)]
    }
    private let items: [Item]

    static func capture() -> PasteboardSnapshot? {
        guard let pasteboardItems = NSPasteboard.general.pasteboardItems,
              !pasteboardItems.isEmpty else { return nil }
        var items: [Item] = []
        for item in pasteboardItems {
            var types: [(NSPasteboard.PasteboardType, Value)] = []
            for type in item.types {
                if let plist = item.propertyList(forType: type) {
                    types.append((type, .plist(plist)))
                } else if let data = item.data(forType: type) {
                    types.append((type, .data(data)))
                } else if let string = item.string(forType: type) {
                    types.append((type, .string(string)))
                }
            }
            if !types.isEmpty {
                items.append(Item(types: types))
            }
        }
        return items.isEmpty ? nil : PasteboardSnapshot(items: items)
    }

    func restore() {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let restored = items.map { item in
            let newItem = NSPasteboardItem()
            for (type, value) in item.types {
                switch value {
                case .plist(let plist): newItem.setPropertyList(plist, forType: type)
                case .data(let data): newItem.setData(data, forType: type)
                case .string(let string): newItem.setString(string, forType: type)
                }
            }
            return newItem
        }
        pasteboard.writeObjects(restored)
    }
}
