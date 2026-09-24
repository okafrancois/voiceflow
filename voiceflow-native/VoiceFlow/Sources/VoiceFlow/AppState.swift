import AppKit
import AVFoundation
import OSLog
import ServiceManagement

let log = Logger(subsystem: "fr.okatech.voiceflow", category: "app")

@MainActor
final class AppState: ObservableObject {
    static let shared = AppState()

    enum Phase {
        case idle, recording, transcribing, polishing
    }

    @Published var phase: Phase = .idle {
        didSet {
            guard phase != oldValue else { return }
            // Direct call: the pill must appear with the sound, not after.
            PillController.shared.apply(phase: phase)
            hotkey.setDictationActive(phase != .idle)
        }
    }
    @Published var volatileTranscript = ""
    @Published var lastTranscript = ""
    @Published var lastError: String?

    /// Permissions, for the onboarding status strip.
    @Published var microphoneGranted = true
    @Published var accessibilityGranted = true

    /// History and statistics, reloaded after each dictation.
    @Published var entries: [HistoryEntry] = []
    @Published var weekUsage = HistoryStore.Usage()
    @Published var todayStats = DayStats()
    @Published var hourlyWords = [Int](repeating: 0, count: 24)

    /// Recent mic levels (0…1), most recent last — for the pill.
    @Published var audioLevels: [Float] = Array(repeating: 0, count: 24)
    @Published var recordingStart: Date?

    /// Dictation language, independent of the system language.
    /// The Mac's language (English, Spanish…) must never dictate the voice
    /// language: it is an explicit user choice.
    /// Special value: automatic language detection (Whisper engines).
    nonisolated static let autoLocaleID = "auto"

    @Published var dictationLocaleID: String = UserDefaults.standard.string(forKey: "dictationLocale") ?? "fr-FR" {
        didSet {
            UserDefaults.standard.set(dictationLocaleID, forKey: "dictationLocale")
            guard dictationLocaleID != oldValue else { return }
            Task { await preloadEngine() }
        }
    }

    /// Apple engine locales, collected at launch.
    @Published var appleLocaleIDs: [String] = []

    /// Languages offered for the active engine.
    var availableLocaleIDs: [String] {
        let ids = engineChoice.supportedLocaleIDs(appleLocales: appleLocaleIDs)
        // French, English, Spanish first; the rest in alphabetical
        // order of their displayed name.
        let priority = ["fr-FR", "fr", "en-US", "en", "es-ES", "es"]
        let head = priority.filter(ids.contains)
        let tail = ids.filter { !priority.contains($0) }
            .sorted { displayName(for: $0) < displayName(for: $1) }
        return head + tail
    }

    /// Chosen transcription engine, persisted.
    @Published var engineChoiceID: String = UserDefaults.standard.string(forKey: "engine") ?? EngineChoice.apple.rawValue {
        didSet {
            UserDefaults.standard.set(engineChoiceID, forKey: "engine")
            // The chosen language may not exist for the new engine.
            if dictationLocaleID != Self.autoLocaleID,
               !availableLocaleIDs.contains(dictationLocaleID) {
                dictationLocaleID = equivalentLocale(dictationLocaleID)
                    ?? availableLocaleIDs.first ?? "en"
            }
            guard engineChoiceID != oldValue else { return }
            Task { await preloadEngine() }
        }
    }

    var engineChoice: EngineChoice {
        EngineChoice(rawValue: engineChoiceID) ?? .apple
    }

    /// Automatic launch at login.
    @Published var launchAtLogin = SMAppService.mainApp.status == .enabled {
        didSet {
            do {
                if launchAtLogin {
                    try SMAppService.mainApp.register()
                } else {
                    try SMAppService.mainApp.unregister()
                }
            } catch {
                lastError = L.t("Lancement à la connexion") + " : \(error.localizedDescription)"
                log.error("login item failed: \(error)")
            }
        }
    }

    /// First launch: onboarding is shown only once.
    @Published var needsOnboarding = !UserDefaults.standard.bool(forKey: "onboarded")

    func completeOnboarding() {
        UserDefaults.standard.set(true, forKey: "onboarded")
        needsOnboarding = false
    }

    /// Interface language. Empty = follow the system. The change takes
    /// effect on the next launch: macOS resolves catalogs at startup.
    @Published var interfaceLanguage: String = UserDefaults.standard.string(forKey: "interfaceLanguage") ?? "" {
        didSet {
            UserDefaults.standard.set(interfaceLanguage, forKey: "interfaceLanguage")
            L.setLanguage(interfaceLanguage)
        }
    }

    /// Appearance of the app and the pill.
    @Published var themeID: String = UserDefaults.standard.string(forKey: "theme") ?? AppTheme.system.rawValue {
        didSet {
            UserDefaults.standard.set(themeID, forKey: "theme")
            theme.apply()
        }
    }

    var theme: AppTheme { AppTheme(rawValue: themeID) ?? .system }

    @Published var pillPositionID: String = UserDefaults.standard.string(forKey: "pillPosition") ?? PillPosition.bottomCenter.rawValue {
        didSet {
            UserDefaults.standard.set(pillPositionID, forKey: "pillPosition")
            PillController.shared.applyAppearance()
        }
    }

    var pillPosition: PillPosition { PillPosition(rawValue: pillPositionID) ?? .bottomCenter }

    @Published var pillVisibilityID: String = UserDefaults.standard.string(forKey: "pillVisibility") ?? PillVisibility.whileActive.rawValue {
        didSet {
            UserDefaults.standard.set(pillVisibilityID, forKey: "pillVisibility")
            PillController.shared.applyAppearance()
        }
    }

    var pillVisibility: PillVisibility { PillVisibility(rawValue: pillVisibilityID) ?? .whileActive }

    @Published var pillTintID: String = UserDefaults.standard.string(forKey: "pillTint") ?? PillTint.dark.rawValue {
        didSet {
            UserDefaults.standard.set(pillTintID, forKey: "pillTint")
            PillController.shared.applyAppearance()
        }
    }

    var pillTint: PillTint { PillTint(rawValue: pillTintID) ?? .dark }

    /// Pill scale (0.875 to 1.375), like the five sizes of the original.
    @Published var pillScale = UserDefaults.standard.object(forKey: "pillScale") as? Double ?? 1 {
        didSet {
            UserDefaults.standard.set(pillScale, forKey: "pillScale")
            PillController.shared.applyAppearance()
        }
    }

    /// Show the live transcription in the pill (Apple engine).
    @Published var showLivePreview = UserDefaults.standard.object(forKey: "showLivePreview") as? Bool ?? true {
        didSet { UserDefaults.standard.set(showLivePreview, forKey: "showLivePreview") }
    }

    /// Recognize "new line", "new paragraph"… within dictation.
    @Published var voiceCommandsEnabled = UserDefaults.standard.object(forKey: "voiceCommands") as? Bool ?? true {
        didSet { UserDefaults.standard.set(voiceCommandsEnabled, forKey: "voiceCommands") }
    }

    /// Join spacing and capitalization with the text preceding the cursor.
    @Published var smartSpacingEnabled = UserDefaults.standard.object(forKey: "smartSpacing") as? Bool ?? true {
        didSet { UserDefaults.standard.set(smartSpacingEnabled, forKey: "smartSpacing") }
    }

    @Published var pillOpacity = UserDefaults.standard.object(forKey: "pillOpacity") as? Double ?? 0.55 {
        didSet {
            UserDefaults.standard.set(pillOpacity, forKey: "pillOpacity")
            PillController.shared.applyAppearance()
        }
    }

    /// Audio input: device, noise reduction, silence trimming.
    @Published var inputDeviceID: AudioDeviceID =
        AudioDeviceID(UserDefaults.standard.integer(forKey: "inputDevice")) {
        didSet { UserDefaults.standard.set(Int(inputDeviceID), forKey: "inputDevice") }
    }

    @Published var noiseReduction = UserDefaults.standard.object(forKey: "noiseReduction") as? Bool ?? false {
        didSet { UserDefaults.standard.set(noiseReduction, forKey: "noiseReduction") }
    }

    @Published var trimSilence = UserDefaults.standard.object(forKey: "trimSilence") as? Bool ?? true {
        didSet { UserDefaults.standard.set(trimSilence, forKey: "trimSilence") }
    }

    /// Required margin above the noise floor. Distinct key from the old
    /// `silenceThreshold`, which was an absolute threshold: values saved
    /// under that key don't mean the same thing and must not be reused.
    @Published var silenceMargin = UserDefaults.standard.object(forKey: "silenceMargin") as? Double ?? 0.05 {
        didSet { UserDefaults.standard.set(silenceMargin, forKey: "silenceMargin") }
    }

    /// Confirmation sounds at the start and end of dictation.
    @Published var soundsEnabled = UserDefaults.standard.object(forKey: "sounds") as? Bool ?? true {
        didSet { UserDefaults.standard.set(soundsEnabled, forKey: "sounds") }
    }

    /// History retention duration, in days; nil = no limit.
    @Published var retentionDays: Int? = UserDefaults.standard.object(forKey: "retentionDays") as? Int {
        didSet {
            if let retentionDays {
                UserDefaults.standard.set(retentionDays, forKey: "retentionDays")
                HistoryStore.shared.deleteOlderThan(days: retentionDays)
                refreshHistory()
            } else {
                UserDefaults.standard.removeObject(forKey: "retentionDays")
            }
        }
    }

    /// Trigger mode (hold / toggle / double tap).
    @Published var triggerMode: TriggerMode = ShortcutSettings.mode {
        didSet {
            ShortcutSettings.mode = triggerMode
            hotkey.reload()
        }
    }

    @Published var dictateShortcut: Shortcut = ShortcutSettings.dictate {
        didSet {
            ShortcutSettings.dictate = dictateShortcut
            hotkey.reload()
        }
    }

    /// Command mode shortcut: none by default.
    @Published var commandShortcut: Shortcut? = ShortcutSettings.command {
        didSet {
            ShortcutSettings.command = commandShortcut
            hotkey.reload()
        }
    }

    /// Optional access to the dictation shortcut, for the shared recorder.
    var dictateShortcutSetting: Shortcut? {
        get { dictateShortcut }
        set { if let newValue { dictateShortcut = newValue } }
    }

    /// Polishing applies to every dictation when enabled — one shortcut,
    /// not two.
    @Published var polishEnabled = UserDefaults.standard.object(forKey: "polishEnabled") as? Bool ?? false {
        didSet { UserDefaults.standard.set(polishEnabled, forKey: "polishEnabled") }
    }

    /// Reinsert into the field that had focus at trigger time, without
    /// reactivating the application. Otherwise, write wherever the cursor is.
    @Published var insertInOriginalField = UserDefaults.standard.object(forKey: "insertInOriginalField") as? Bool ?? true {
        didSet { UserDefaults.standard.set(insertInOriginalField, forKey: "insertInOriginalField") }
    }

    /// Polish style (templates ported from the Tauri app), persisted.
    @Published var polishTemplateID: String = UserDefaults.standard.string(forKey: "polishTemplate") ?? "filler" {
        didSet { UserDefaults.standard.set(polishTemplateID, forKey: "polishTemplate") }
    }

    /// Automatic detection chosen while the engine can't do it.
    var autoLanguageUnavailable: Bool {
        dictationLocaleID == Self.autoLocaleID && !engineChoice.detectsLanguage
    }

    /// Is something blocking dictation (missing permission)?
    var needsAttention: Bool {
        !microphoneGranted || !accessibilityGranted
    }

    /// Instruction shown on the home screen, based on the trigger mode.
    var triggerHint: String {
        let keys = dictateShortcut.display
        let base = switch triggerMode {
        case .hold: L.t("Maintenez la combinaison en parlant, puis relâchez pour insérer.")
        case .toggle: L.t("Pressez pour démarrer, à nouveau pour insérer.")
        case .doubleTap: L.t("Double-pressez pour démarrer, une fois pour insérer.")
        }
        let suffix = polishEnabled
            ? " " + L.t("Le texte est poli avant l'insertion.")
            : ""
        return "\(keys) · " + base + suffix
    }

    var readinessTitle: String {
        if !microphoneGranted { return L.t("Configurez votre microphone") }
        if !accessibilityGranted { return L.t("Autorisez l'accessibilité") }
        switch phase {
        case .idle where isPreparingModel: return L.t("Préparation du modèle…")
        case .recording: return L.t("Enregistrement en cours")
        case .transcribing: return L.t("Transcription…")
        case .polishing: return L.t("Polissage…")
        case .idle: return L.t("Prêt à dicter")
        }
    }

    /// Finds the same language in the current engine's list: "fr-FR"
    /// becomes "fr" with Whisper, and vice versa.
    private func equivalentLocale(_ localeID: String) -> String? {
        let code = Locale(identifier: localeID).language.languageCode?.identifier
        return availableLocaleIDs.first {
            Locale(identifier: $0).language.languageCode?.identifier == code
        }
    }

    /// Name of a language, in the interface language.
    func displayName(for localeID: String) -> String {
        if localeID == Self.autoLocaleID {
            return L.t("Détection automatique")
        }
        let name = L.locale.localizedString(forIdentifier: localeID) ?? localeID
        return name.prefix(1).capitalized + name.dropFirst()
    }

    var menuBarSymbol: String {
        if isPreparingModel, phase == .idle { return "arrow.down.circle" }
        switch phase {
        case .idle: return "waveform"
        case .recording: return "waveform.circle.fill"
        case .transcribing: return "ellipsis.circle"
        case .polishing: return "sparkles"
        }
    }

    /// Model currently downloading or loading. Doesn't prevent dictating:
    /// the audio waits for the engine (see `EngineFeed`).
    @Published var isPreparingModel = false

    let hotkey = HotkeyManager()
    /// On-device polishing, by the system model.
    let polisher = FoundationModelsPolisher()
    /// Ongoing dictation, from trigger to insertion.
    var session: DictationSession?
    /// Last insertion, so it can be undone.
    @Published var lastInsertion: InsertionRecord?
    /// Brief message shown by the pill (error, cancellation…). Errors also
    /// remain in `lastError`, readable in settings.
    @Published var notice: Notice?
    var preparingCount = 0
    var maintenanceTask: Task<Void, Never>?

    func bootstrap() async {
        L.setLanguage(interfaceLanguage)
        theme.apply()
        PillController.shared.attach(to: self)
        refreshHistory()

        refreshPermissions()
        if !accessibilityGranted {
            log.warning("accessibility not yet granted; hotkey and injection degraded")
        }

        // Re-evaluate when the user comes back from System Settings.
        NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil, queue: .main
        ) { _ in
            Task { @MainActor in AppState.shared.refreshPermissions() }
        }

        hotkey.onStart = { [weak self] kind in self?.startDictation(kind: kind) }
        hotkey.onStop = { [weak self] in
            Task { @MainActor in await self?.stopDictation() }
        }
        hotkey.onCancel = { [weak self] reason in
            guard let self else { return }
            // A chord (Fn + ↑) only cancels a recording; during processing,
            // it concerns the app, not the previous dictation.
            if reason == .chord, self.phase != .recording { return }
            self.cancelDictation()
        }
        hotkey.start()

        applyRetention()
        scheduleDailyMaintenance()

        let supported = await TranscriptionSession.supportedLocales()
            .map { $0.identifier(.bcp47) }
        appleLocaleIDs = supported

        // Load the chosen engine so the first dictation doesn't pay the wait.
        await preloadEngine()

        await UpdateChecker.shared.checkIfDue()
    }
}
