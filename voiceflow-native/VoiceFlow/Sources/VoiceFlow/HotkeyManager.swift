import AppKit
import CoreGraphics

/// Global shortcuts. Active CGEventTap to swallow the combination (it
/// must not leak into the target app); falls back to a passive NSEvent
/// listener if the tap fails, for lack of Accessibility permission.
///
/// The tap runs on its own thread. Until its callback has responded,
/// macOS holds back *all* keyboard input for the session: when placed
/// on the main thread, any UI stall (another app being read via
/// accessibility, the history database…) froze the keyboard, then macOS
/// cut the tap and the key release leaked into the app.
///
/// The commands (`onStart`, `onStop`, `onCancel`) are delivered on the
/// main thread.
final class HotkeyManager {
    /// What a shortcut triggers.
    enum Action {
        /// Ordinary dictation. Polishing depends on the setting, not the shortcut.
        case dictate
        /// Command mode: an instruction applied to the selected text.
        case command
    }

    /// Start a dictation of the given kind.
    var onStart: (Action) -> Void = { _ in }
    /// Stop and insert.
    var onStop: () -> Void = {}
    /// Why a cancellation is being requested.
    enum CancelReason {
        /// Escape: also applies during processing.
        case escape
        /// Single key used within a combination (Fn + ↑): only applies to a
        /// recording that just started.
        case chord
    }

    /// Abandon the current dictation.
    var onCancel: (CancelReason) -> Void = { _ in }

    private static let escapeKeyCode: UInt16 = 53

    /// Protects everything read by both the tap thread and the main thread.
    private let lock = NSLock()

    /// A shortcut and the machine that interprets its presses.
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
    /// A dictation is in progress: Escape cancels it instead of leaking into the app.
    private var cancelArmed = false
    private var swallowedEscape = false

    /// Capture mode: while recording a new shortcut, the tap intercepts the
    /// combination instead of triggering dictation. Without this, pressing
    /// the current shortcut would start a dictation and the key would never
    /// reach the interface.
    private var captureHandler: ((UInt16, NSEvent.ModifierFlags) -> Void)?
    /// Modifier held down during a capture: it's only validated as a
    /// standalone shortcut on release, otherwise the ⌥ in an ⌥J combination
    /// would be captured before the J is even pressed.
    private var capturePendingModifier: UInt16?

    private var eventTap: CFMachPort?
    private var globalMonitors: [Any] = []
    private(set) var isTapActive = false

    /// Call after settings change.
    func reload() {
        lock.withLock { bindings = Self.loadBindings() }
    }

    /// The app reports its state: Escape is only intercepted during a
    /// dictation, and the machine resyncs when the app returns to idle on
    /// its own.
    func setDictationActive(_ active: Bool) {
        lock.withLock {
            cancelArmed = active
            if !active {
                for index in bindings.indices { bindings[index].machine.dictationEnded() }
            }
        }
    }

    /// The app refused a start (missing model, dictation already in
    /// progress…): the machines must not believe a dictation started,
    /// otherwise the next press would be taken as a stop.
    func startRejected() {
        lock.withLock {
            for index in bindings.indices { bindings[index].machine.dictationEnded() }
        }
    }

    /// Captures the next combination instead of triggering dictation.
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

    /// Interception can only start with Accessibility permission. Granted
    /// during onboarding, it arrives *after* launch: a retry is therefore
    /// needed, otherwise the shortcut stays dead until the next launch.
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
        // The port is only touched by this thread from here on, and by
        // `tapEnable`, documented as safe from any thread.
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

        // Our own simulated keystrokes (keyboard insertion, ⌘V) are not
        // user gestures.
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

    // MARK: - Events

    /// Each handler returns: whether to swallow the event, and what to do
    /// once the lock is released (never an outgoing call while locked).
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
                    // Pressed: might be the start of a combination, so we wait.
                    capturePendingModifier = keyCode
                    return (swallowFn, {})
                }
                // Released without any key following: this is a single-key
                // shortcut.
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

    /// The tap was cut off: a release may have been lost. Re-read the
    /// keyboard's real state instead of letting a dictation run unattended.
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

    /// Translates a machine action into a deferred call.
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

    /// The app's callbacks are set once at launch and are only called on
    /// the main thread: this hop is what gets them there.
    private static func onMain(_ work: @escaping () -> Void) {
        if Thread.isMainThread {
            work()
        } else {
            nonisolated(unsafe) let work = work
            DispatchQueue.main.async { work() }
        }
    }

    // MARK: - Passive fallback

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
