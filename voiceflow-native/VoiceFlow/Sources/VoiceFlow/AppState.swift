import AppKit
import AVFoundation
import OSLog
import ServiceManagement

let log = Logger(subsystem: "fr.okatech.voiceflow", category: "app")

@MainActor
final class AppState: ObservableObject {
    static let shared = AppState()

    enum Phase {
        case idle, preparing, recording, transcribing, polishing
    }

    @Published var phase: Phase = .idle {
        didSet {
            guard phase != oldValue else { return }
            // Appel direct : la pill doit apparaître avec le son, pas après.
            PillController.shared.apply(phase: phase)
        }
    }
    @Published var volatileTranscript = ""
    @Published var lastTranscript = ""
    @Published var lastError: String?

    /// Permissions, pour la bande d'état de l'accueil.
    @Published var microphoneGranted = true
    @Published var accessibilityGranted = true

    /// Historique et statistiques, rechargés après chaque dictée.
    @Published var entries: [HistoryEntry] = []
    @Published var weekUsage = HistoryStore.Usage()
    @Published var todayStats = DayStats()
    @Published var hourlyWords = [Int](repeating: 0, count: 24)

    /// Niveaux micro récents (0…1), le plus frais en dernier — pour la pill.
    @Published var audioLevels: [Float] = Array(repeating: 0, count: 24)
    @Published var recordingStart: Date?

    /// Langue de dictée, indépendante de la langue du système.
    /// La langue du Mac (anglais, espagnol…) ne doit jamais dicter la langue
    /// de la voix : c'est un choix explicite de l'utilisateur.
    /// Valeur spéciale : détection automatique de la langue (moteurs Whisper).
    static let autoLocaleID = "auto"

    @Published var dictationLocaleID: String = UserDefaults.standard.string(forKey: "dictationLocale") ?? "fr-FR" {
        didSet {
            UserDefaults.standard.set(dictationLocaleID, forKey: "dictationLocale")
            guard dictationLocaleID != Self.autoLocaleID else { return }
            let locale = Locale(identifier: dictationLocaleID)
            Task {
                do {
                    try await TranscriptionSession.prepareAssets(for: locale)
                } catch {
                    await MainActor.run {
                        self.lastError = "Modèle \(self.displayName(for: self.dictationLocaleID)) indisponible : \(error.localizedDescription)"
                    }
                }
            }
        }
    }

    /// Locales du moteur d'Apple, relevées au lancement.
    @Published var appleLocaleIDs: [String] = []

    /// Langues proposées pour le moteur actif.
    var availableLocaleIDs: [String] {
        let ids = engineChoice.supportedLocaleIDs(appleLocales: appleLocaleIDs)
        // Français, anglais, espagnol en tête ; le reste par ordre
        // alphabétique de leur nom affiché.
        let priority = ["fr-FR", "fr", "en-US", "en", "es-ES", "es"]
        let head = priority.filter(ids.contains)
        let tail = ids.filter { !priority.contains($0) }
            .sorted { displayName(for: $0) < displayName(for: $1) }
        return head + tail
    }

    /// Moteur de transcription choisi, persistant.
    @Published var engineChoiceID: String = UserDefaults.standard.string(forKey: "engine") ?? EngineChoice.apple.rawValue {
        didSet {
            UserDefaults.standard.set(engineChoiceID, forKey: "engine")
            // La langue choisie peut ne pas exister pour le nouveau moteur.
            if dictationLocaleID != Self.autoLocaleID,
               !availableLocaleIDs.contains(dictationLocaleID) {
                dictationLocaleID = equivalentLocale(dictationLocaleID)
                    ?? availableLocaleIDs.first ?? "en"
            }
        }
    }

    var engineChoice: EngineChoice {
        EngineChoice(rawValue: engineChoiceID) ?? .apple
    }

    /// Lancement automatique à l'ouverture de session.
    @Published var launchAtLogin = SMAppService.mainApp.status == .enabled {
        didSet {
            do {
                if launchAtLogin {
                    try SMAppService.mainApp.register()
                } else {
                    try SMAppService.mainApp.unregister()
                }
            } catch {
                lastError = "Lancement à la connexion : \(error.localizedDescription)"
                log.error("login item failed: \(error)")
            }
        }
    }

    /// Premier lancement : l'onboarding ne s'affiche qu'une fois.
    @Published var needsOnboarding = !UserDefaults.standard.bool(forKey: "onboarded")

    func completeOnboarding() {
        UserDefaults.standard.set(true, forKey: "onboarded")
        needsOnboarding = false
    }

    /// Langue de l'interface. Vide = suivre le système. Le changement prend
    /// effet au prochain lancement : macOS résout les catalogues au démarrage.
    @Published var interfaceLanguage: String = UserDefaults.standard.string(forKey: "interfaceLanguage") ?? "" {
        didSet {
            UserDefaults.standard.set(interfaceLanguage, forKey: "interfaceLanguage")
            L.setLanguage(interfaceLanguage)
        }
    }

    /// Apparence de l'app et de la pill.
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

    /// Échelle de la pill (0,875 à 1,375), comme les cinq tailles de l'original.
    @Published var pillScale = UserDefaults.standard.object(forKey: "pillScale") as? Double ?? 1 {
        didSet {
            UserDefaults.standard.set(pillScale, forKey: "pillScale")
            PillController.shared.applyAppearance()
        }
    }

    @Published var pillOpacity = UserDefaults.standard.object(forKey: "pillOpacity") as? Double ?? 0.55 {
        didSet {
            UserDefaults.standard.set(pillOpacity, forKey: "pillOpacity")
            PillController.shared.applyAppearance()
        }
    }

    /// Entrée audio : périphérique, réduction de bruit, coupe du silence.
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

    /// Écart exigé au-dessus du plancher de bruit. Clé distincte de l'ancien
    /// `silenceThreshold`, qui était un seuil absolu : les valeurs enregistrées
    /// alors n'ont pas le même sens et ne doivent pas être reprises.
    @Published var silenceMargin = UserDefaults.standard.object(forKey: "silenceMargin") as? Double ?? 0.05 {
        didSet { UserDefaults.standard.set(silenceMargin, forKey: "silenceMargin") }
    }

    /// Sons de confirmation au début et à la fin de la dictée.
    @Published var soundsEnabled = UserDefaults.standard.object(forKey: "sounds") as? Bool ?? true {
        didSet { UserDefaults.standard.set(soundsEnabled, forKey: "sounds") }
    }

    /// Durée de conservation de l'historique, en jours ; nil = sans limite.
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

    /// Mode de déclenchement (maintenir / basculer / double appui).
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

    /// Le polissage s'applique à chaque dictée quand il est activé — un seul
    /// raccourci, pas deux.
    @Published var polishEnabled = UserDefaults.standard.object(forKey: "polishEnabled") as? Bool ?? false {
        didSet { UserDefaults.standard.set(polishEnabled, forKey: "polishEnabled") }
    }

    /// Réinsérer dans le champ qui avait le focus au déclenchement, sans
    /// réactiver l'application. Sinon, on écrit là où est le curseur.
    @Published var insertInOriginalField = UserDefaults.standard.object(forKey: "insertInOriginalField") as? Bool ?? true {
        didSet { UserDefaults.standard.set(insertInOriginalField, forKey: "insertInOriginalField") }
    }

    /// Style de polissage (templates portés de l'app Tauri), persistant.
    @Published var polishTemplateID: String = UserDefaults.standard.string(forKey: "polishTemplate") ?? "filler" {
        didSet { UserDefaults.standard.set(polishTemplateID, forKey: "polishTemplate") }
    }

    /// Détection automatique choisie alors que le moteur ne sait pas la faire.
    var autoLanguageUnavailable: Bool {
        dictationLocaleID == Self.autoLocaleID && !engineChoice.detectsLanguage
    }

    /// Quelque chose bloque la dictée (permission manquante) ?
    var needsAttention: Bool {
        !microphoneGranted || !accessibilityGranted
    }

    /// Instruction affichée sur l'accueil, selon le mode de déclenchement.
    var triggerHint: String {
        let keys = dictateShortcut.display
        let base = switch triggerMode {
        case .hold: String(localized: "Maintenez la combinaison en parlant, puis relâchez pour insérer.")
        case .toggle: String(localized: "Pressez pour démarrer, à nouveau pour insérer.")
        case .doubleTap: String(localized: "Double-pressez pour démarrer, une fois pour insérer.")
        }
        let suffix = polishEnabled
            ? " " + String(localized: "Le texte est poli avant l'insertion.")
            : ""
        return "\(keys) · " + base + suffix
    }

    var readinessTitle: String {
        if !microphoneGranted { return "Configurez votre microphone" }
        if !accessibilityGranted { return "Autorisez l'accessibilité" }
        switch phase {
        case .preparing: return "Préparation du modèle…"
        case .recording: return "Enregistrement en cours"
        case .transcribing: return "Transcription…"
        case .polishing: return "Polissage…"
        case .idle: return "Prêt à dicter"
        }
    }

    /// Retrouve la même langue dans la liste du moteur courant : « fr-FR »
    /// devient « fr » avec Whisper, et inversement.
    private func equivalentLocale(_ localeID: String) -> String? {
        let code = Locale(identifier: localeID).language.languageCode?.identifier
        return availableLocaleIDs.first {
            Locale(identifier: $0).language.languageCode?.identifier == code
        }
    }

    func displayName(for localeID: String) -> String {
        if localeID == Self.autoLocaleID {
            return "Détection automatique"
        }
        let french = Locale(identifier: "fr_FR")
        let name = french.localizedString(forIdentifier: localeID) ?? localeID
        return name.prefix(1).capitalized + name.dropFirst()
    }

    var menuBarSymbol: String {
        switch phase {
        case .idle: "waveform"
        case .preparing: "arrow.down.circle"
        case .recording: "waveform.circle.fill"
        case .transcribing: "ellipsis.circle"
        case .polishing: "sparkles"
        }
    }

    private let hotkey = HotkeyManager()
    /// Polissage sur l'appareil, par le modèle du système.
    private let polisher = FoundationModelsPolisher()
    private var recorder: AudioRecorder?
    private var engine: DictationEngine?
    private var capturedTarget: CapturedTextTarget?
    private var pendingPolish = false

    func bootstrap() async {
        L.setLanguage(interfaceLanguage)
        theme.apply()
        PillController.shared.attach(to: self)
        refreshHistory()

        refreshPermissions()
        if !accessibilityGranted {
            log.warning("accessibility not yet granted; hotkey and injection degraded")
        }

        // Réévaluer quand l'utilisateur revient des Réglages Système.
        NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: nil, queue: .main
        ) { _ in
            Task { @MainActor in AppState.shared.refreshPermissions() }
        }

        hotkey.onStart = { [weak self] in
            Task { @MainActor in self?.startDictation() }
        }
        hotkey.onStop = { [weak self] in
            Task { @MainActor in await self?.stopDictation() }
        }
        hotkey.start()

        // Appliquer la rétention au lancement.
        if let days = retentionDays {
            HistoryStore.shared.deleteOlderThan(days: days)
        }

        let supported = await TranscriptionSession.supportedLocales()
            .map { $0.identifier(.bcp47) }
        appleLocaleIDs = supported

        // Pré-télécharge le modèle de la langue choisie pour ne pas payer
        // l'attente à la première dictée.
        phase = .preparing
        do {
            try await TranscriptionSession.prepareAssets(for: Locale(identifier: dictationLocaleID))
        } catch {
            lastError = "Modèle de transcription indisponible : \(error.localizedDescription)"
            log.error("asset preparation failed: \(error)")
        }
        phase = .idle

        await UpdateChecker.shared.checkIfDue()
    }

    func toggleDictation() {
        if phase == .recording {
            Task { await stopDictation() }
        } else if phase == .idle {
            startDictation()
        }
    }

    func startDictation() {
        guard phase == .idle else { return }
        guard microphoneGranted else {
            // Jamais demandé (onboarding passé) : poser la question maintenant.
            Task {
                if await requestMicrophone() { startDictation() }
            }
            return
        }
        lastError = nil
        volatileTranscript = ""
        pendingPolish = polishEnabled

        // Retour immédiat d'abord. Le préchargement du modèle et la capture
        // d'accessibilité sont synchrones et peuvent bloquer plusieurs
        // secondes — la capture interroge une autre application —, ce qui
        // retardait l'apparition de la pill.
        phase = .recording
        playSound(start: true)
        Diagnostics.log("dictée démarrée · moteur \(engineChoice.rawValue) · langue \(dictationLocaleID) · polissage \(polishEnabled)")

        // La pill ne prend pas le focus : capturer juste après reste correct.
        capturedTarget = CapturedTextTarget(captureAccessibility: insertInOriginalField)
        if polishEnabled {
            let template = PolishCatalog.resolved(polishTemplateID)
            Task.detached(priority: .utility) { [polisher] in
                polisher.prewarm(template: template)
            }
        }
        Task {
            do {
                let isAuto = dictationLocaleID == Self.autoLocaleID
                let engine: DictationEngine
                if let whisperModel = engineChoice.whisperModel {
                    // Un modèle anglais seul ne sait rien détecter d'autre :
                    // lui laisser deviner la langue produirait du charabia.
                    let language: String? = engineChoice.isEnglishOnly
                        ? "en"
                        : (isAuto
                            ? nil
                            : Locale(identifier: dictationLocaleID).language.languageCode?.identifier)
                    engine = try WhisperEngine(model: whisperModel, language: language)
                } else if let sherpa = engineChoice.sherpaModel {
                    engine = try SherpaEngine(
                        model: sherpa,
                        language: isAuto
                            ? nil
                            : Locale(identifier: dictationLocaleID).language.languageCode?.identifier)
                } else {
                    // Le moteur système exige une langue explicite.
                    let locale = Locale(identifier: isAuto ? "fr-FR" : dictationLocaleID)
                    engine = try await TranscriptionSession(locale: locale) { volatile in
                        Task { @MainActor in self.volatileTranscript = volatile }
                    }
                }
                self.engine = engine

                let recorder = AudioRecorder()
                try recorder.start(
                    options: AudioRecorder.Options(
                        deviceID: inputDeviceID,
                        noiseReduction: noiseReduction,
                        trimSilence: trimSilence,
                        silenceMargin: Float(silenceMargin)),
                    onBuffer: { buffer in
                        engine.feed(buffer)
                    },
                    onLevel: { level in
                        Task { @MainActor in self.pushLevel(level) }
                    }
                )
                self.recorder = recorder
                self.recordingStart = Date()
                log.info("dictation started (\(self.engineChoice.rawValue))")
            } catch {
                self.phase = .idle
                self.lastError = "Démarrage impossible : \(error.localizedDescription)"
                log.error("start failed: \(error)")
            }
        }
    }

    func stopDictation() async {
        guard phase == .recording, let engine else { return }
        let shouldPolish = pendingPolish
        pendingPolish = false
        phase = .transcribing
        playSound(start: false)
        let peak = recorder?.peakLevel ?? 0
        recorder?.stop()
        recorder = nil
        let audioDurationMs = Int((recordingStart.map { -$0.timeIntervalSinceNow } ?? 0) * 1000)
        recordingStart = nil
        audioLevels = Array(repeating: 0, count: audioLevels.count)

        // Silence absolu : inutile d'interroger le moteur, et surtout il faut
        // le dire — macOS ne signale pas une autorisation micro manquante,
        // il livre simplement des blocs vides.
        Diagnostics.log("prise terminée · \(audioDurationMs) ms · crête \(String(format: "%.3f", peak))")
        guard peak > 0.001 else {
            self.engine = nil
            phase = .idle
            microphoneGranted = Permissions.isMicrophoneGranted()
            lastError = microphoneGranted
                ? L.t("Aucun son capté. Vérifiez que le micro choisi est le bon et qu'il n'est pas coupé.")
                : L.t("Aucun son capté : VoiceFlow n'a pas l'autorisation micro. Réglages Système › Confidentialité › Microphone.")
            log.error("no audio captured (peak 0) — microphone granted: \(self.microphoneGranted)")
            return
        }

        do {
            let sttStart = Date()
            let text = try await engine.finish()
            let sttDurationMs = Int(-sttStart.timeIntervalSinceNow * 1000)
            self.engine = nil
            volatileTranscript = ""
            phase = .idle

            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                // Silence complet plutôt qu'échec : le dire, sinon l'app a
                // l'air de tourner dans le vide.
                Diagnostics.log("transcription vide (coupe du silence : \(trimSilence))")
            lastError = trimSilence
                    ? L.t("Aucune parole reconnue. Si cela se répète, baissez la sensibilité de la coupe du silence, ou désactivez-la.")
                    : L.t("Aucune parole reconnue.")
                log.info("empty transcription (trimSilence=\(self.trimSilence))")
                return
            }

            // Dictionnaire et extraits avant tout le reste.
            let corrected = VocabularyStore.shared.apply(to: trimmed)

            var final = corrected
            var polishDurationMs: Int?
            if shouldPolish {
                phase = .polishing
                let polishStart = Date()
                // Une règle d'application l'emporte sur le style par défaut.
                let templateID = VocabularyStore.shared
                    .templateID(forBundleID: capturedTarget?.bundleID) ?? polishTemplateID
                do {
                    final = try await polisher.polish(
                        corrected, template: PolishCatalog.resolved(templateID),
                        locale: dictationLocaleID == Self.autoLocaleID ? nil : dictationLocaleID)
                    polishDurationMs = Int(-polishStart.timeIntervalSinceNow * 1000)
                    log.info("polished (\(templateID))")
                } catch {
                    // Le polissage ne doit jamais faire perdre la dictée :
                    // on insère le texte brut et on signale l'échec.
                    lastError = "Polissage ignoré, texte brut inséré : \(error.localizedDescription)"
                    log.error("polish failed: \(error)")
                }
                phase = .idle
            }
            Diagnostics.log("transcrit \(trimmed.count) caractères en \(sttDurationMs) ms")
            lastTranscript = final
            insert(final)
            CorrectionWatcher.watch(inserted: final, in: capturedTarget?.accessibility)

            HistoryStore.shared.insert(
                rawText: trimmed, finalText: final,
                appName: capturedTarget?.appName,
                engine: engineChoice.historyEngine,
                model: engineChoice.historyModel,
                language: dictationLocaleID,
                audioDurationMs: audioDurationMs, sttDurationMs: sttDurationMs,
                polishDurationMs: polishDurationMs,
                polishEngine: polishDurationMs == nil ? nil : "foundation-models")
            refreshHistory()
        } catch {
            self.engine = nil
            phase = .idle
            Diagnostics.log("échec transcription : \(error.localizedDescription)")
            lastError = "Transcription échouée : \(error.localizedDescription)"
            log.error("transcription failed: \(error)")
        }
    }

    /// Vrai si l'interception clavier globale fonctionne (permission
    /// Accessibilité accordée).
    var hotkeyTapActive: Bool { hotkey.isTapActive }

    /// Enregistre la prochaine combinaison pressée dans le raccourci donné.
    func captureShortcut(into keyPath: ReferenceWritableKeyPath<AppState, Shortcut>,
                         completion: @escaping () -> Void) {
        hotkey.beginCapture { [weak self] keyCode, flags in
            Task { @MainActor in
                guard let self else { return }
                defer { completion() }
                // Échap annule. Une touche seule n'est acceptée que si elle ne
                // sert pas à écrire : modificateur (Fn, ⌘…) ou touche de fonction.
                guard keyCode != 53 else { return }
                guard Shortcut.isAssignable(keyCode: keyCode, modifiers: flags) else {
                    self.lastError = "Raccourci refusé : une touche ordinaire seule serait avalée partout. Utilisez Fn, une touche de fonction, ou ajoutez un modificateur."
                    return
                }
                let relevant = flags.intersection([.control, .option, .shift, .command])
                self[keyPath: keyPath] = Shortcut(keyCode: keyCode, modifiers: relevant.rawValue)
            }
        }
    }

    func cancelShortcutCapture() {
        hotkey.endCapture()
    }

    func refreshHistory() {
        entries = HistoryStore.shared.recent()
        let today = HistoryStore.shared.todayStats()
        todayStats = today.stats
        hourlyWords = today.hourly
        weekUsage = HistoryStore.shared.usage(days: 7).usage
    }

    /// Demande l'accès micro (boîte système, une seule fois dans la vie de
    /// l'app). Appelé depuis l'onboarding ou à la première dictée.
    @discardableResult
    func requestMicrophone() async -> Bool {
        let granted = await Permissions.requestMicrophone()
        microphoneGranted = granted
        if !granted {
            lastError = "Accès micro refusé — Réglages Système › Confidentialité › Microphone"
        }
        return granted
    }

    /// Affiche l'invite d'accessibilité du système.
    func requestAccessibility() {
        Permissions.ensureAccessibility()
        refreshPermissions()
    }

    /// Réévalue les permissions (à l'ouverture de la fenêtre).
    func refreshPermissions() {
        let wasGranted = accessibilityGranted
        accessibilityGranted = Permissions.isAccessibilityTrusted()
        microphoneGranted = Permissions.isMicrophoneGranted()

        // L'autorisation vient d'arriver : mettre en place l'interception.
        if accessibilityGranted, !wasGranted {
            hotkey.restartIfNeeded()
            log.info("accessibility granted, hotkey restarted")
        }
    }

    func deleteEntry(_ entry: HistoryEntry) {
        HistoryStore.shared.delete(id: entry.id)
        refreshHistory()
    }

    func clearHistory() {
        HistoryStore.shared.deleteAll()
        refreshHistory()
    }

    func insertEntry(_ entry: HistoryEntry) {
        insert(entry.finalText)
    }

    private func playSound(start: Bool) {
        guard soundsEnabled else { return }
        BeepPlayer.shared.play(start: start)
    }

    private func pushLevel(_ level: Float) {
        audioLevels.removeFirst()
        audioLevels.append(level)
    }

    func reinsertLast() {
        guard !lastTranscript.isEmpty else { return }
        insert(lastTranscript)
    }

    private func insert(_ text: String) {
        // Priorité à la cible accessibilité capturée au démarrage (insertion
        // sans réactiver l'app) ; sinon injection dans le focus courant.
        if let target = capturedTarget {
            do {
                _ = try target.insertBackground(text)
                Diagnostics.log("inséré dans le champ d'origine")
                return
            } catch {
                log.info("background insert unavailable (\(error.localizedDescription)), falling back")
            }
        }
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                let method = try TextInjector.insert(text)
                log.info("inserted via \(String(describing: method))")
            } catch {
                Task { @MainActor in
                    self.lastError = "Insertion échouée : \(error.localizedDescription)"
                }
                log.error("injection failed: \(error)")
            }
        }
    }
}
