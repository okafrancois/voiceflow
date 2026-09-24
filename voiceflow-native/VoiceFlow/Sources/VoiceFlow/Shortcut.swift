import AppKit
import Carbon.HIToolbox

/// Un raccourci global : une touche et ses modificateurs.
struct Shortcut: Codable, Equatable {
    var keyCode: UInt16
    /// Masque `NSEvent.ModifierFlags` restreint aux modificateurs utiles.
    var modifiers: UInt

    static let dictateDefault = Shortcut(keyCode: UInt16(kVK_Space), modifiers: NSEvent.ModifierFlags.option.rawValue)

    var flags: NSEvent.ModifierFlags { NSEvent.ModifierFlags(rawValue: modifiers) }

    // MARK: Touches seules

    /// Codes des touches modificatrices : elles peuvent servir de raccourci à
    /// elles seules (Fn, ⌘, ⌥…), comme le double-appui sur Fn.
    static let modifierKeyCodes: Set<UInt16> = [54, 55, 56, 57, 58, 59, 60, 61, 62, 63]

    static let functionKeyCodes: Set<UInt16> = [
        122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111,  // F1–F12
        105, 107, 113, 106, 64, 79, 80, 90,                       // F13–F20
    ]

    /// Raccourci composé d'une seule touche modificatrice.
    var isModifierOnly: Bool {
        Self.modifierKeyCodes.contains(keyCode) && flags.isEmpty
    }

    /// Le drapeau correspondant à une touche modificatrice donnée.
    static func flag(for keyCode: UInt16) -> NSEvent.ModifierFlags? {
        switch keyCode {
        case 54, 55: .command
        case 56, 60: .shift
        case 57: .capsLock
        case 58, 61: .option
        case 59, 62: .control
        case 63: .function
        default: nil
        }
    }

    /// Bits propres à chaque côté du clavier (`NX_DEVICE…KEYMASK`), présents
    /// dans les drapeaux bruts. Le drapeau générique `.command` reste levé
    /// tant que l'un *ou* l'autre ⌘ est tenu : ⌘ droite relâché pendant que
    /// ⌘ gauche est enfoncé passerait sinon pour toujours pressé.
    private static func sideMask(for keyCode: UInt16) -> UInt? {
        switch keyCode {
        case 59: 0x0001  // ⌃ gauche
        case 56: 0x0002  // ⇧ gauche
        case 60: 0x0004  // ⇧ droite
        case 55: 0x0008  // ⌘ gauche
        case 54: 0x0010  // ⌘ droite
        case 58: 0x0020  // ⌥ gauche
        case 61: 0x0040  // ⌥ droite
        case 62: 0x2000  // ⌃ droite
        default: nil
        }
    }

    /// La touche modificatrice est-elle enfoncée dans cet état de drapeaux ?
    static func isPressed(_ keyCode: UInt16, _ flags: NSEvent.ModifierFlags) -> Bool {
        guard let flag = flag(for: keyCode), flags.contains(flag) else { return false }
        guard let side = sideMask(for: keyCode) else { return true }
        // Événement synthétique sans bits de côté : s'en tenir au générique.
        let anySide: UInt = 0x207F
        guard flags.rawValue & anySide != 0 else { return true }
        return flags.rawValue & side != 0
    }

    /// Fn est avalée quand elle sert de raccourci, sinon macOS ouvre le
    /// sélecteur d'emoji. Les autres modificateurs doivent passer : ils
    /// servent en permanence dans d'autres combinaisons.
    static func shouldSwallow(_ keyCode: UInt16) -> Bool { keyCode == 63 }

    /// Peut-on assigner cette combinaison sans casser la frappe normale ?
    /// Une lettre seule serait avalée partout ; une touche de fonction ou un
    /// modificateur seul, non.
    static func isAssignable(keyCode: UInt16, modifiers: NSEvent.ModifierFlags) -> Bool {
        let relevant = modifiers.intersection([.control, .option, .shift, .command])
        if !relevant.isEmpty { return true }
        return modifierKeyCodes.contains(keyCode) || functionKeyCodes.contains(keyCode)
    }

    /// Rendu type menu macOS : ⌃⌥⇧⌘ puis la touche.
    var display: String {
        if isModifierOnly {
            switch keyCode {
            case 63: return "Fn"
            case 54: return L.t("⌘ droite")
            case 55: return "⌘"
            case 56: return "⇧"
            case 57: return "⇪"
            case 58: return "⌥"
            case 59: return "⌃"
            case 60: return L.t("⇧ droite")
            case 61: return L.t("⌥ droite")
            case 62: return L.t("⌃ droite")
            default: return Self.unknownKey(keyCode)
            }
        }
        var text = ""
        if flags.contains(.control) { text += "⌃" }
        if flags.contains(.option) { text += "⌥" }
        if flags.contains(.shift) { text += "⇧" }
        if flags.contains(.command) { text += "⌘" }
        return text + Self.keyName(keyCode)
    }

    func matches(keyCode: UInt16, flags: NSEvent.ModifierFlags) -> Bool {
        // Un raccourci « touche modificatrice seule » se reconnaît sur les
        // changements de drapeaux, pas sur les frappes ordinaires.
        guard !isModifierOnly else { return false }
        let relevant: NSEvent.ModifierFlags = [.control, .option, .shift, .command]
        return keyCode == self.keyCode && flags.intersection(relevant) == self.flags.intersection(relevant)
    }

    private static func unknownKey(_ keyCode: UInt16) -> String {
        String(format: L.t("Touche %d"), Int(keyCode))
    }

    static func keyName(_ keyCode: UInt16) -> String {
        switch Int(keyCode) {
        case kVK_Space: return L.t("Espace")
        case kVK_Return: return L.t("Retour")
        case kVK_Tab: return L.t("Tab")
        case kVK_Escape: return L.t("Échap")
        case kVK_Delete: return L.t("Suppr")
        case kVK_F1: return "F1"
        case kVK_F2: return "F2"
        case kVK_F3: return "F3"
        case kVK_F4: return "F4"
        case kVK_F5: return "F5"
        case kVK_F6: return "F6"
        case kVK_F7: return "F7"
        case kVK_F8: return "F8"
        case kVK_F9: return "F9"
        case kVK_F10: return "F10"
        case kVK_F11: return "F11"
        case kVK_F12: return "F12"
        case kVK_ANSI_Grave: return "`"
        default: break
        }
        // Lettre ou chiffre : demander au clavier courant.
        guard let source = TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue(),
              let pointer = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData)
        else { return unknownKey(keyCode) }
        let data = Unmanaged<CFData>.fromOpaque(pointer).takeUnretainedValue() as Data
        var deadKeys: UInt32 = 0
        var length = 0
        var characters = [UniChar](repeating: 0, count: 4)
        let status = data.withUnsafeBytes { buffer -> OSStatus in
            guard let layout = buffer.bindMemory(to: UCKeyboardLayout.self).baseAddress else {
                return OSStatus(-1)
            }
            return UCKeyTranslate(
                layout, keyCode, UInt16(kUCKeyActionDisplay), 0,
                UInt32(LMGetKbdType()), OptionBits(kUCKeyTranslateNoDeadKeysBit),
                &deadKeys, characters.count, &length, &characters)
        }
        guard status == noErr, length > 0 else { return unknownKey(keyCode) }
        return String(utf16CodeUnits: characters, count: length).uppercased()
    }
}

/// Comment le raccourci déclenche la dictée.
enum TriggerMode: String, CaseIterable, Identifiable, Codable {
    case hold, toggle, doubleTap
    var id: String { rawValue }

    var title: String {
        switch self {
        case .hold: L.t("Maintenir")
        case .toggle: L.t("Basculer")
        case .doubleTap: L.t("Double appui")
        }
    }

    var help: String {
        switch self {
        case .hold: L.t("Parlez en maintenant les touches, relâchez pour insérer.")
        case .toggle: L.t("Une pression démarre, une autre arrête.")
        case .doubleTap: L.t("Deux pressions rapides démarrent, une pression arrête.")
        }
    }
}

/// Réglages de raccourcis, persistés.
enum ShortcutSettings {
    static var dictate: Shortcut {
        get { load("shortcutDictate") ?? .dictateDefault }
        set { store(newValue, "shortcutDictate") }
    }

    /// Raccourci du mode commande ; aucun par défaut.
    static var command: Shortcut? {
        get { load("shortcutCommand") }
        set {
            if let newValue {
                store(newValue, "shortcutCommand")
            } else {
                UserDefaults.standard.removeObject(forKey: "shortcutCommand")
            }
        }
    }

    static var mode: TriggerMode {
        get {
            UserDefaults.standard.string(forKey: "triggerMode")
                .flatMap(TriggerMode.init(rawValue:)) ?? .hold
        }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: "triggerMode") }
    }

    private static func load(_ key: String) -> Shortcut? {
        guard let data = UserDefaults.standard.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(Shortcut.self, from: data)
    }

    private static func store(_ shortcut: Shortcut, _ key: String) {
        guard let data = try? JSONEncoder().encode(shortcut) else { return }
        UserDefaults.standard.set(data, forKey: key)
    }
}
