import AppKit
import ApplicationServices
import CoreGraphics

/// Port Swift de `apps/desktop/src-tauri/src/text_injector/` (Rust), qui fait
/// référence pour le comportement :
/// - cible AX capturée au démarrage de la dictée → insertion sans réactiver l'app ;
/// - sinon : multiligne ou > 400 graphèmes → presse-papiers + Cmd+V (avec
///   sauvegarde/restauration complète du presse-papiers) ; texte court → frappe
///   clavier simulée par tranches de 100 graphèmes espacées de 50 ms.

enum InjectionMethod {
    case keyboard, clipboard, accessibility
}

enum InjectionError: LocalizedError {
    case noCapturedTarget
    case attributeNotWritable(String)
    case axRejected(String, AXError)
    case keyboardAndClipboardFailed
    case clipboardPasteFailed

    var errorDescription: String? {
        switch self {
        case .noCapturedTarget:
            "Le champ d'origine n'a pas été capturé"
        case .attributeNotWritable(let attribute):
            "Le champ d'origine n'est pas modifiable (\(attribute))"
        case .axRejected(let attribute, let code):
            "Le champ d'origine a refusé l'insertion (\(attribute), erreur \(code.rawValue))"
        case .keyboardAndClipboardFailed:
            "Injection clavier et presse-papiers échouées"
        case .clipboardPasteFailed:
            "Collage via le presse-papiers échoué"
        }
    }
}

// MARK: - Cible accessibilité

/// Instantané du champ qui a le focus au démarrage de l'enregistrement :
/// élément AX + position du curseur. Permet d'insérer plus tard sans
/// réactiver l'application.
struct AccessibilityTarget {
    private let element: AXUIElement
    private let selectedRange: CFRange?

    static func capture() -> AccessibilityTarget? {
        let system = AXUIElementCreateSystemWide()
        var focusedValue: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(
            system, kAXFocusedUIElementAttribute as CFString, &focusedValue)
        guard error == .success, let focusedValue else { return nil }
        let element = focusedValue as! AXUIElement

        guard isSettable(element, kAXSelectedTextAttribute) else { return nil }

        var selectedRange: CFRange?
        if isSettable(element, kAXSelectedTextRangeAttribute) {
            selectedRange = copyRange(element, kAXSelectedTextRangeAttribute)
        }
        return AccessibilityTarget(element: element, selectedRange: selectedRange)
    }

    /// Relit le contenu du champ, pour repérer une correction après coup.
    func readValue() -> String? {
        var value: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(
            element, kAXValueAttribute as CFString, &value)
        guard error == .success else { return nil }
        return value as? String
    }

    func insert(_ text: String) throws {
        if let selectedRange {
            try setRange(element, kAXSelectedTextRangeAttribute, selectedRange)
        }
        try setString(element, kAXSelectedTextAttribute, text)
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

/// Cible capturée au moment où la dictée démarre : le champ éditable et
/// l'application au premier plan (qui détermine le style de polissage).
final class CapturedTextTarget {
    let accessibility: AccessibilityTarget?
    let bundleID: String?
    let appName: String?

    init(captureAccessibility: Bool) {
        accessibility = captureAccessibility ? AccessibilityTarget.capture() : nil
        let frontmost = NSWorkspace.shared.frontmostApplication
        bundleID = frontmost?.bundleIdentifier
        appName = frontmost?.localizedName
    }

    func insertBackground(_ text: String) throws -> InjectionMethod {
        guard let accessibility else {
            throw InjectionError.noCapturedTarget
        }
        try accessibility.insert(text)
        return .accessibility
    }
}

// MARK: - Injection dans le focus courant

enum TextInjector {
    private static let chunkSize = 100
    private static let chunkDelayMs: UInt64 = 50
    private static let clipboardThreshold = 400

    static func insert(_ text: String) throws -> InjectionMethod {
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

    // MARK: Couche 0 — frappe clavier simulée

    /// Simule la frappe sans toucher au presse-papiers. Tranches de
    /// 100 graphèmes avec 50 ms de pause pour ne pas casser la composition IME ;
    /// chaque CGEvent porte au plus 20 unités UTF-16.
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
            down.keyboardSetUnicodeString(stringLength: slice.count, unicodeString: slice)
            up.keyboardSetUnicodeString(stringLength: slice.count, unicodeString: slice)
            down.post(tap: .cghidEventTap)
            up.post(tap: .cghidEventTap)
            start += 20
        }
        return true
    }

    // MARK: Couche 2 — presse-papiers + Cmd+V

    private static func clipboardPaste(_ text: String) -> Bool {
        let saved = DispatchQueue.main.sync { PasteboardSnapshot.capture() }
        DispatchQueue.main.sync {
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString(text, forType: .string)
        }

        let ok = pressCmdV()

        // Le collage est rapide (~10-20 ms) : restaurer peu après.
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(100)) {
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

    private static func pressCmdV() -> Bool {
        guard let source = CGEventSource(stateID: .combinedSessionState) else {
            return false
        }
        // Laisser l'app cible reprendre le focus avant de recevoir les touches.
        usleep(20_000)

        // 0x09 = 'v' physique, indépendant de la disposition clavier.
        guard
            let down = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: true),
            let up = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: false)
        else { return false }
        down.flags = .maskCommand
        up.flags = .maskCommand
        down.post(tap: .cghidEventTap)
        up.post(tap: .cghidEventTap)
        return true
    }
}

// MARK: - Sauvegarde/restauration du presse-papiers

/// Copie intégrale du presse-papiers (tous items, tous types), restituée
/// après le collage pour que l'utilisateur ne perde rien.
private struct PasteboardSnapshot {
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
