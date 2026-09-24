import AppKit
import CoreGraphics

/// Raccourcis globaux. CGEventTap actif pour avaler la combinaison (elle ne
/// doit pas partir dans l'app cible) ; repli en écoute passive NSEvent si le
/// tap échoue, faute d'autorisation Accessibilité.
///
/// Le tap tourne sur son propre fil. Tant que son rappel n'a pas répondu,
/// macOS retient *toute* la saisie clavier de la session : posé sur le fil
/// principal, chaque blocage de l'interface (lecture d'une autre app par
/// l'accessibilité, base d'historique…) gelait le clavier, puis macOS coupait
/// le tap et le relâchement de la touche partait dans l'app.
///
/// Les ordres (`onStart`, `onStop`, `onCancel`) sont livrés sur le fil
/// principal.
final class HotkeyManager {
    /// Ce que déclenche un raccourci.
    enum Action {
        /// Dictée ordinaire. Le polissage dépend du réglage, pas du raccourci.
        case dictate
        /// Mode commande : une consigne appliquée au texte sélectionné.
        case command
    }

    /// Démarrer une dictée du type donné.
    var onStart: (Action) -> Void = { _ in }
    /// Arrêter et insérer.
    var onStop: () -> Void = {}
    /// Pourquoi une annulation est demandée.
    enum CancelReason {
        /// Échap : vaut aussi pendant le traitement.
        case escape
        /// Touche seule utilisée dans une combinaison (Fn + ↑) : ne concerne
        /// qu'un enregistrement qui vient de démarrer.
        case chord
    }

    /// Abandonner la dictée en cours.
    var onCancel: (CancelReason) -> Void = { _ in }

    private static let escapeKeyCode: UInt16 = 53

    /// Protège tout ce que lisent à la fois le fil du tap et le fil principal.
    private let lock = NSLock()

    /// Un raccourci et la machine qui interprète ses pressions.
    private struct Binding {
        let action: Action
        let shortcut: Shortcut
        var machine: TriggerStateMachine
    }
    private var bindings = HotkeyManager.loadBindings()

    private static func loadBindings() -> [Binding] {
        let mode = ShortcutSettings.mode
        var shortcuts: [(Action, Shortcut)] = [(.dictate, ShortcutSettings.dictate)]
        if let command = ShortcutSettings.command, command != ShortcutSettings.dictate {
            shortcuts.append((.command, command))
        }
        return shortcuts.map {
            Binding(action: $0.0, shortcut: $0.1,
                    machine: TriggerStateMachine(mode: mode, modifierOnly: $0.1.isModifierOnly))
        }
    }
    /// Une dictée est en cours : Échap l'annule au lieu de partir dans l'app.
    private var cancelArmed = false
    private var swallowedEscape = false

    /// Mode capture : pendant l'enregistrement d'un nouveau raccourci, le tap
    /// intercepte la combinaison au lieu de déclencher la dictée. Sans cela,
    /// presser le raccourci actuel lancerait une dictée et la touche
    /// n'atteindrait jamais l'interface.
    private var captureHandler: ((UInt16, NSEvent.ModifierFlags) -> Void)?
    /// Modificateur enfoncé pendant une capture : on ne le valide comme
    /// raccourci à lui seul qu'au relâchement, sinon ⌥ d'une combinaison ⌥J
    /// serait capturé avant même la frappe du J.
    private var capturePendingModifier: UInt16?

    private var eventTap: CFMachPort?
    private var globalMonitors: [Any] = []
    private(set) var isTapActive = false

    /// À appeler après modification des réglages.
    func reload() {
        lock.withLock { bindings = Self.loadBindings() }
    }

    /// L'app signale son état : Échap n'est intercepté que pendant une
    /// dictée, et la machine se recale quand l'app revient d'elle-même au
    /// repos.
    func setDictationActive(_ active: Bool) {
        lock.withLock {
            cancelArmed = active
            if !active {
                for index in bindings.indices { bindings[index].machine.dictationEnded() }
            }
        }
    }

    /// L'app a refusé un démarrage (modèle absent, dictée déjà en cours…) :
    /// les machines ne doivent pas croire une dictée lancée, sinon la
    /// pression suivante serait prise pour un arrêt.
    func startRejected() {
        lock.withLock {
            for index in bindings.indices { bindings[index].machine.dictationEnded() }
        }
    }

    /// Capture la prochaine combinaison au lieu de déclencher la dictée.
    func beginCapture(_ handler: @escaping (UInt16, NSEvent.ModifierFlags) -> Void) {
        lock.withLock {
            captureHandler = handler
            for index in bindings.indices { bindings[index].machine.reset() }
        }
    }

    func endCapture() {
        lock.withLock {
            captureHandler = nil
            capturePendingModifier = nil
        }
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
        // Le port n'est plus touché que par ce fil, et par `tapEnable`,
        // documenté comme sûr depuis n'importe quel fil.
        nonisolated(unsafe) let port = tap
        let thread = Thread {
            let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, port, 0)
            CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .commonModes)
            CGEvent.tapEnable(tap: port, enable: true)
            CFRunLoopRun()
        }
        thread.name = "fr.okatech.voiceflow.hotkey"
        thread.qualityOfService = .userInteractive
        thread.start()
        return true
    }

    private func handle(type: CGEventType, event: CGEvent) -> Unmanaged<CGEvent>? {
        let pass = Unmanaged.passUnretained(event)
        let keyCode = UInt16(event.getIntegerValueField(.keyboardEventKeycode))
        let flags = NSEvent.ModifierFlags(rawValue: UInt(event.flags.rawValue))
        let now = Date()

        // Nos propres frappes simulées (insertion au clavier, ⌘V) ne sont
        // pas des gestes de l'utilisateur.
        if type == .keyDown || type == .keyUp || type == .flagsChanged,
           event.getIntegerValueField(.eventSourceUnixProcessID) == Int64(getpid()) {
            return pass
        }

        switch type {
        case .tapDisabledByTimeout, .tapDisabledByUserInput:
            if let eventTap { CGEvent.tapEnable(tap: eventTap, enable: true) }
            reconcileHeldKey(at: now)
            return pass

        case .keyDown:
            let isRepeat = event.getIntegerValueField(.keyboardEventAutorepeat) != 0
            let (swallow, work) = keyDown(keyCode: keyCode, flags: flags, isRepeat: isRepeat, at: now)
            work()
            return swallow ? nil : pass

        case .keyUp:
            let (swallow, work) = keyUp(keyCode: keyCode, at: now)
            work()
            return swallow ? nil : pass

        case .flagsChanged:
            let (swallow, work) = flagsChanged(keyCode: keyCode, flags: flags, at: now)
            work()
            return swallow ? nil : pass

        default:
            return pass
        }
    }

    // MARK: - Événements

    /// Chaque gestionnaire rend : faut-il avaler l'événement, et quoi faire
    /// une fois le verrou relâché (jamais d'appel sortant sous verrou).
    private typealias Outcome = (swallow: Bool, work: () -> Void)

    private func keyDown(
        keyCode: UInt16, flags: NSEvent.ModifierFlags, isRepeat: Bool, at now: Date
    ) -> Outcome {
        lock.withLock {
            if let capture = captureHandler {
                guard !isRepeat else { return (true, {}) }
                captureHandler = nil
                capturePendingModifier = nil
                return (true, { Self.onMain { capture(keyCode, flags) } })
            }

            let relevant = flags.intersection([.control, .option, .shift, .command])
            if cancelArmed, keyCode == Self.escapeKeyCode, relevant.isEmpty {
                guard !isRepeat else { return (true, {}) }
                swallowedEscape = true
                return (true, { [weak self] in
                    Self.onMain { self?.onCancel(.escape) }
                })
            }

            if let index = bindings.firstIndex(where: { $0.shortcut.matches(keyCode: keyCode, flags: flags) }) {
                guard !isRepeat else { return (true, {}) }
                return (true, emit(bindings[index].machine.press(at: now), for: bindings[index].action))
            }
            var work: [() -> Void] = []
            for index in bindings.indices {
                work.append(emit(bindings[index].machine.otherKey(at: now), for: bindings[index].action))
            }
            return (false, { work.forEach { $0() } })
        }
    }

    private func keyUp(keyCode: UInt16, at now: Date) -> Outcome {
        lock.withLock {
            if captureHandler != nil { return (true, {}) }
            if keyCode == Self.escapeKeyCode, swallowedEscape {
                swallowedEscape = false
                return (true, {})
            }
            if let index = bindings.firstIndex(where: {
                !$0.shortcut.isModifierOnly && $0.shortcut.keyCode == keyCode && $0.machine.isHeld
            }) {
                return (true, emit(bindings[index].machine.release(at: now), for: bindings[index].action))
            }
            return (false, {})
        }
    }

    private func flagsChanged(
        keyCode: UInt16, flags: NSEvent.ModifierFlags, at now: Date
    ) -> Outcome {
        lock.withLock {
            let swallow = Shortcut.shouldSwallow(keyCode)
                && bindings.contains { $0.shortcut.keyCode == keyCode }

            if let capture = captureHandler {
                guard Shortcut.modifierKeyCodes.contains(keyCode) else { return (false, {}) }
                let swallowFn = Shortcut.shouldSwallow(keyCode)
                if Shortcut.isPressed(keyCode, flags) {
                    // Enfoncé : peut-être le début d'une combinaison, on attend.
                    capturePendingModifier = keyCode
                    return (swallowFn, {})
                }
                // Relâché sans qu'aucune touche n'ait suivi : c'est un
                // raccourci à touche unique.
                let relevant = flags.intersection([.control, .option, .shift, .command])
                guard capturePendingModifier == keyCode, relevant.isEmpty else {
                    return (swallowFn, {})
                }
                captureHandler = nil
                capturePendingModifier = nil
                return (swallowFn, { Self.onMain { capture(keyCode, []) } })
            }

            guard let index = bindings.firstIndex(where: {
                $0.shortcut.isModifierOnly && $0.shortcut.keyCode == keyCode
            }) else {
                return (false, {})
            }
            let action = Shortcut.isPressed(keyCode, flags)
                ? bindings[index].machine.press(at: now)
                : bindings[index].machine.release(at: now)
            return (swallow, emit(action, for: bindings[index].action))
        }
    }

    /// Le tap a été coupé : un relâchement a pu se perdre. Relire l'état
    /// réel du clavier plutôt que de laisser une dictée tourner seule.
    private func reconcileHeldKey(at now: Date) {
        let work: [() -> Void] = lock.withLock {
            bindings.indices.compactMap { index in
                let shortcut = bindings[index].shortcut
                guard bindings[index].machine.isHeld else { return nil }
                let stillPressed: Bool
                if shortcut.isModifierOnly {
                    let raw = CGEventSource.flagsState(.combinedSessionState).rawValue
                    stillPressed = Shortcut.isPressed(
                        shortcut.keyCode, NSEvent.ModifierFlags(rawValue: UInt(raw)))
                } else {
                    stillPressed = CGEventSource.keyState(
                        .combinedSessionState, key: CGKeyCode(shortcut.keyCode))
                }
                guard !stillPressed else { return nil }
                log.warning("hotkey: release lost while the tap was disabled, recovering")
                return emit(bindings[index].machine.release(at: now), for: bindings[index].action)
            }
        }
        work.forEach { $0() }
    }

    /// Traduit une action de la machine en appel différé.
    private func emit(
        _ action: TriggerStateMachine.Action?, for kind: Action = .dictate
    ) -> () -> Void {
        guard let action else { return {} }
        return { [weak self] in
            Self.onMain {
                guard let self else { return }
                switch action {
                case .start: self.onStart(kind)
                case .stop: self.onStop()
                case .cancel: self.onCancel(.chord)
                }
            }
        }
    }

    /// Les rappels de l'app sont posés une fois au lancement et ne sont
    /// appelés que sur le fil principal : c'est ce saut qui les y amène.
    private static func onMain(_ work: @escaping () -> Void) {
        if Thread.isMainThread {
            work()
        } else {
            nonisolated(unsafe) let work = work
            DispatchQueue.main.async { work() }
        }
    }

    // MARK: - Repli passif

    private func startFallbackMonitor() {
        let down = NSEvent.addGlobalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self else { return }
            self.keyDown(
                keyCode: event.keyCode, flags: event.modifierFlags,
                isRepeat: event.isARepeat, at: Date()
            ).work()
        }
        let up = NSEvent.addGlobalMonitorForEvents(matching: .keyUp) { [weak self] event in
            self?.keyUp(keyCode: event.keyCode, at: Date()).work()
        }
        let flags = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            self?.flagsChanged(keyCode: event.keyCode, flags: event.modifierFlags, at: Date()).work()
        }
        globalMonitors = [down, up, flags].compactMap { $0 }
    }
}
