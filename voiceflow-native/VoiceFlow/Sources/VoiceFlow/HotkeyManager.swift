import AppKit
import CoreGraphics

/// Raccourcis globaux. CGEventTap actif pour avaler la combinaison (elle ne
/// doit pas partir dans l'app cible) ; repli en écoute passive NSEvent si le
/// tap échoue, faute d'autorisation Accessibilité.
final class HotkeyManager {
    /// Démarrer la dictée. Le polissage dépend du réglage, pas du raccourci.
    var onStart: () -> Void = {}
    /// Arrêter et insérer.
    var onStop: () -> Void = {}

    private var eventTap: CFMachPort?
    private var globalMonitors: [Any] = []
    private var isHeld = false
    private var isActive = false
    private var lastTap: (date: Date, keyCode: UInt16)?
    /// Certains claviers émettent deux événements pour une seule pression de
    /// modificateur : on ignore le doublon immédiat.
    private var lastPress: (date: Date, keyCode: UInt16)?

    /// Mode capture : pendant l'enregistrement d'un nouveau raccourci, le tap
    /// intercepte la combinaison au lieu de déclencher la dictée. Sans cela,
    /// presser le raccourci actuel lancerait une dictée et la touche
    /// n'atteindrait jamais l'interface.
    private var captureHandler: ((UInt16, NSEvent.ModifierFlags) -> Void)?
    /// Modificateur enfoncé pendant une capture : on ne le valide comme
    /// raccourci à lui seul qu'au relâchement, sinon ⌥ d'une combinaison ⌥J
    /// serait capturé avant même la frappe du J.
    private var capturePendingModifier: UInt16?
    private(set) var isTapActive = false

    private var dictate = ShortcutSettings.dictate
    private var mode = ShortcutSettings.mode

    /// À appeler après modification des réglages.
    func reload() {
        dictate = ShortcutSettings.dictate
        mode = ShortcutSettings.mode
        isHeld = false
    }

    /// Capture la prochaine combinaison au lieu de déclencher la dictée.
    /// `handler` reçoit `nil` si l'utilisateur annule avec Échap.
    func beginCapture(_ handler: @escaping (UInt16, NSEvent.ModifierFlags) -> Void) {
        captureHandler = handler
        isHeld = false
        isActive = false
    }

    func endCapture() {
        captureHandler = nil
        capturePendingModifier = nil
    }

    /// L'interception ne peut naître qu'avec l'autorisation Accessibilité.
    /// Accordée pendant l'onboarding, elle arrive *après* le lancement : il
    /// faut donc retenter, sinon le raccourci reste mort jusqu'au prochain
    /// démarrage.
    func restartIfNeeded() {
        guard !isTapActive else { return }
        globalMonitors.forEach(NSEvent.removeMonitor)
        globalMonitors = []
        start()
    }

    func start() {
        if startEventTap() {
            isTapActive = true
            log.info("hotkey: CGEventTap active")
        } else {
            startFallbackMonitor()
            log.warning("hotkey: CGEventTap unavailable, using passive NSEvent monitor")
        }
    }

    // MARK: - CGEventTap

    private func startEventTap() -> Bool {
        let mask: CGEventMask =
            (1 << CGEventType.keyDown.rawValue)
            | (1 << CGEventType.keyUp.rawValue)
            | (1 << CGEventType.flagsChanged.rawValue)

        let callback: CGEventTapCallBack = { _, type, event, userInfo in
            guard let userInfo else { return Unmanaged.passUnretained(event) }
            let manager = Unmanaged<HotkeyManager>.fromOpaque(userInfo).takeUnretainedValue()
            return manager.handle(type: type, event: event)
        }

        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: mask,
            callback: callback,
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else { return false }

        eventTap = tap
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        return true
    }

    private func handle(type: CGEventType, event: CGEvent) -> Unmanaged<CGEvent>? {
        switch type {
        case .tapDisabledByTimeout, .tapDisabledByUserInput:
            if let eventTap { CGEvent.tapEnable(tap: eventTap, enable: true) }
            return Unmanaged.passUnretained(event)

        case .keyDown:
            let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
            let isRepeat = event.getIntegerValueField(.keyboardEventAutorepeat) != 0
            let flags = NSEvent.ModifierFlags(rawValue: UInt(event.flags.rawValue))

            if let capture = captureHandler {
                guard !isRepeat else { return nil }
                captureHandler = nil
                capturePendingModifier = nil
                capture(keyCode, flags)
                return nil // la frappe sert à définir le raccourci, rien d'autre
            }

            if dictate.matches(keyCode: keyCode, flags: flags) {
                if !isRepeat { keyDown(keyCode: keyCode) }
                return nil // avalé
            }
            return Unmanaged.passUnretained(event)

        case .keyUp:
            let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
            if captureHandler != nil { return nil }
            if isHeld, keyCode == dictate.keyCode {
                keyUp()
                return nil
            }
            return Unmanaged.passUnretained(event)

        case .flagsChanged:
            let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
            let flags = NSEvent.ModifierFlags(rawValue: UInt(event.flags.rawValue))
            let passThrough = Shortcut.shouldSwallow(keyCode)
                ? nil : Unmanaged.passUnretained(event)

            if let capture = captureHandler {
                guard Shortcut.modifierKeyCodes.contains(keyCode) else {
                    return Unmanaged.passUnretained(event)
                }
                if Shortcut.isPressed(keyCode, flags) {
                    // Enfoncé : peut-être le début d'une combinaison, on attend.
                    capturePendingModifier = keyCode
                    return passThrough
                }
                // Relâché sans qu'aucune touche n'ait suivi : c'est un
                // raccourci à touche unique.
                let relevant = flags.intersection([.control, .option, .shift, .command])
                guard capturePendingModifier == keyCode, relevant.isEmpty else {
                    return passThrough
                }
                captureHandler = nil
                capturePendingModifier = nil
                capture(keyCode, [])
                return passThrough
            }

            guard dictate.isModifierOnly, dictate.keyCode == keyCode else {
                return Unmanaged.passUnretained(event)
            }
            if Shortcut.isPressed(keyCode, flags) {
                keyDown(keyCode: keyCode)
            } else if isHeld {
                keyUp()
            }
            return passThrough

        default:
            return Unmanaged.passUnretained(event)
        }
    }

    // MARK: - Logique des modes

    private func keyDown(keyCode: UInt16) {
        let now = Date()
        if let lastPress, lastPress.keyCode == keyCode,
           now.timeIntervalSince(lastPress.date) < 0.08 {
            return
        }
        lastPress = (now, keyCode)

        switch mode {
        case .hold:
            guard !isHeld else { return }
            isHeld = true
            isActive = true
            onStart()

        case .toggle:
            if isActive {
                isActive = false
                onStop()
            } else {
                isActive = true
                onStart()
            }

        case .doubleTap:
            if isActive {
                isActive = false
                onStop()
                lastTap = nil
                return
            }
            if let last = lastTap, last.keyCode == keyCode,
               now.timeIntervalSince(last.date) < 0.4 {
                lastTap = nil
                isActive = true
                onStart()
            } else {
                lastTap = (now, keyCode)
            }
        }
    }

    private func keyUp() {
        isHeld = false
        guard mode == .hold, isActive else { return }
        isActive = false
        onStop()
    }

    // MARK: - Repli passif

    private func startFallbackMonitor() {
        let down = NSEvent.addGlobalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, !event.isARepeat,
                  self.dictate.matches(keyCode: event.keyCode, flags: event.modifierFlags)
            else { return }
            self.keyDown(keyCode: event.keyCode)
        }
        let up = NSEvent.addGlobalMonitorForEvents(matching: .keyUp) { [weak self] event in
            guard let self, self.isHeld, event.keyCode == self.dictate.keyCode else { return }
            self.keyUp()
        }
        let flags = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            guard let self, self.dictate.isModifierOnly, self.dictate.keyCode == event.keyCode
            else { return }
            if Shortcut.isPressed(event.keyCode, event.modifierFlags) {
                self.keyDown(keyCode: event.keyCode)
            } else if self.isHeld {
                self.keyUp()
            }
        }
        globalMonitors = [down, up, flags].compactMap { $0 }
    }
}
