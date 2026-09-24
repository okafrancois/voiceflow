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

// MARK: - File d'accessibilité

/// Tous les échanges d'accessibilité avec une autre app passent par ici.
///
/// Chaque appel attend la réponse de l'app interrogée ; sur le fil principal,
/// une app lente figeait l'interface — et, avant que le raccourci n'ait son
/// propre fil, le clavier entier. Une file série garde aussi l'ordre :
/// capture, insertion, relecture.
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

// MARK: - Cible accessibilité

/// Instantané du champ qui a le focus au démarrage de l'enregistrement :
/// élément AX + position du curseur. Permet d'insérer plus tard sans
/// réactiver l'application.
/// `AXUIElement` est une référence Core Foundation utilisable depuis
/// n'importe quel fil ; les échanges eux-mêmes passent par `AXQueue`.
struct AccessibilityTarget: @unchecked Sendable {
    private let element: AXUIElement
    private let selectedRange: CFRange?

    /// Délai maximal accordé à l'app interrogée. Par défaut macOS attend
    /// six secondes une app qui ne répond pas.
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

    /// Relit le contenu du champ, pour repérer une correction après coup.
    func readValue() -> String? {
        var value: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(
            element, kAXValueAttribute as CFString, &value)
        guard error == .success else { return nil }
        return value as? String
    }

    /// Insère à la position capturée et rend cette position (en unités
    /// UTF-16, celles de l'accessibilité), si elle est connue.
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
            // Délai dépassé : l'app a pu écrire quand même. Vérifier avant de
            // se replier, sinon le texte arriverait deux fois.
            log.warning("accessibility write timed out, verifying")
            timedOut = true
        }

        // Chromium/Electron déclarent l'attribut modifiable et renvoient
        // .success sans rien insérer : relire le champ pour s'en assurer.
        // L'écriture y est asynchrone, d'où quelques relectures espacées.
        for attempt in 0..<Self.verifyAttempts {
            switch Self.verdict(before: before, after: readValue()) {
            case .landed: return location
            // Sans délai dépassé, l'accessibilité a dit oui : la croire.
            // Après un délai dépassé et un champ illisible, rien ne prouve
            // l'écriture : mieux vaut un repli qu'une dictée perdue.
            case .unknown: if !timedOut { return location }
            case .ignored: break
            }
            if attempt < Self.verifyAttempts - 1 {
                usleep(Self.verifyDelayUs)
            }
        }
        throw InjectionError.writeIgnored
    }

    /// Retire un texte inséré plus tôt, à condition qu'il soit encore
    /// intact à sa place : jamais d'effacement à l'aveugle. `restoring`
    /// reprend sa place (la sélection que l'insertion avait remplacée).
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

    /// Le texte qui précède la position d'insertion, pour décider de
    /// l'espace et de la majuscule. `nil` si le champ ne le dit pas.
    func textBeforeInsertion(maxLength: Int = 64) -> String? {
        guard let value = readValue() as NSString? else { return nil }
        let location = selectedRange?.location
            ?? Self.copyRange(element, kAXSelectedTextRangeAttribute)?.location
        guard let location, location >= 0, location <= value.length else { return nil }
        let start = max(0, location - maxLength)
        return value.substring(with: NSRange(location: start, length: location - start))
    }

    /// Le texte sélectionné au moment de la capture (mode commande).
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

    /// Contenu inchangé après l'écriture : le champ l'a ignorée. Contenu
    /// illisible : impossible de trancher, on fait confiance au succès AX.
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

/// Cible capturée au moment où la dictée démarre : le champ éditable et
/// l'application au premier plan (qui détermine le style de polissage).
final class CapturedTextTarget: Sendable {
    let accessibility: AccessibilityTarget?
    let bundleID: String?
    let appName: String?
    /// Texte sélectionné au déclenchement (mode commande).
    let selection: String?

    init(accessibility: AccessibilityTarget?, bundleID: String?, appName: String?,
         selection: String? = nil) {
        self.accessibility = accessibility
        self.bundleID = bundleID
        self.appName = appName
        self.selection = selection
    }

    /// L'app au premier plan se lit tout de suite ; le champ, lui, demande
    /// d'interroger l'app cible, donc sur la file d'accessibilité.
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

// MARK: - Injection dans le focus courant

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
            // Sans drapeaux explicites, l'événement hérite des touches encore
            // tenues (le ⌥ du raccourci) et l'app peut y voir un raccourci.
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

    // MARK: Couche 2 — presse-papiers + Cmd+V

    /// Délai avant de rendre le presse-papiers. Une app Electron chargée
    /// lit le contenu bien après le ⌘V ; restauré trop tôt (100 ms
    /// auparavant), c'est l'ancien contenu qu'elle collait.
    private static let restoreDelay: DispatchTimeInterval = .milliseconds(700)

    /// Types reconnus par les gestionnaires de presse-papiers
    /// (nspasteboard.org) : contenu éphémère, à ne pas archiver.
    private static let transientTypes: [NSPasteboard.PasteboardType] = [
        NSPasteboard.PasteboardType("org.nspasteboard.TransientType"),
        NSPasteboard.PasteboardType("org.nspasteboard.ConcealedType"),
        NSPasteboard.PasteboardType("org.nspasteboard.AutoGeneratedType"),
    ]

    /// Restauration en attente : un second collage avant qu'elle n'ait lieu
    /// doit reprendre le contenu d'origine, pas notre propre texte.
    /// Lu et écrit uniquement sur le fil principal.
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
            // Un collage plus récent s'occupera de la restauration.
            let isLatest = MainActor.assumeIsolated {
                guard restoreGeneration == generation else { return false }
                pendingRestore = nil
                return true
            }
            guard isLatest else { return }
            // Quelqu'un a copié entre-temps : son contenu prime sur le nôtre.
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

    /// ⌘Z dans l'app au premier plan.
    static func pressUndo() {
        waitForModifierRelease()
        // 0x06 = 'z' physique.
        _ = pressKey(0x06, flags: .maskCommand)
    }

    /// 0x09 = 'v', 0x06 = 'z' : touches physiques, indépendantes de la
    /// disposition clavier.
    private static func pressKey(_ key: CGKeyCode, flags: CGEventFlags) -> Bool {
        guard let source = CGEventSource(stateID: .combinedSessionState) else {
            return false
        }
        // Laisser l'app cible reprendre le focus avant de recevoir les touches.
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

    /// Attend que l'utilisateur ait lâché ⌘ ⌥ ⌃ ⇧ (une seconde au plus) :
    /// une frappe simulée pendant que le ⌥ du raccourci est encore tenu
    /// devient un caractère spécial ou un raccourci de l'app.
    static func waitForModifierRelease(timeout: TimeInterval = 1.0) {
        let held: CGEventFlags = [.maskCommand, .maskAlternate, .maskControl, .maskShift]
        let deadline = Date().addingTimeInterval(timeout)
        while !CGEventSource.flagsState(.combinedSessionState).intersection(held).isEmpty,
              Date() < deadline {
            usleep(10_000)
        }
    }
}

// MARK: - Sauvegarde/restauration du presse-papiers

/// Copie intégrale du presse-papiers (tous items, tous types), restituée
/// après le collage pour que l'utilisateur ne perde rien.
///
/// Créée et restituée sur le fil principal uniquement ; elle ne fait que
/// transiter par le fil d'insertion.
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
