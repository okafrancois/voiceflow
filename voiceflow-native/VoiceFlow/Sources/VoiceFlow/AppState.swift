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
            // Appel direct : la pill doit apparaître avec le son, pas après.
            PillController.shared.apply(phase: phase)
            hotkey.setDictationActive(phase != .idle)
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
    nonisolated static let autoLocaleID = "auto"

    @Published var dictationLocaleID: String = UserDefaults.standard.string(forKey: "dictationLocale") ?? "fr-FR" {
        didSet {
            UserDefaults.standard.set(dictationLocaleID, forKey: "dictationLocale")
            guard dictationLocaleID != oldValue else { return }
            Task { await preloadEngine() }
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
            guard engineChoiceID != oldValue else { return }
            Task { await preloadEngine() }
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
                lastError = L.t("Lancement à la connexion") + " : \(error.localizedDescription)"
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

    /// Afficher la transcription en direct dans la pill (moteur d'Apple).
    @Published var showLivePreview = UserDefaults.standard.object(forKey: "showLivePreview") as? Bool ?? true {
        didSet { UserDefaults.standard.set(showLivePreview, forKey: "showLivePreview") }
    }

    /// Reconnaître « à la ligne », « nouveau paragraphe »… dans la dictée.
    @Published var voiceCommandsEnabled = UserDefaults.standard.object(forKey: "voiceCommands") as? Bool ?? true {
        didSet { UserDefaults.standard.set(voiceCommandsEnabled, forKey: "voiceCommands") }
    }

    /// Raccorder espace et majuscule au texte qui précède le curseur.
    @Published var smartSpacingEnabled = UserDefaults.standard.object(forKey: "smartSpacing") as? Bool ?? true {
        didSet { UserDefaults.standard.set(smartSpacingEnabled, forKey: "smartSpacing") }
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

    /// Raccourci du mode commande : aucun par défaut.
    @Published var commandShortcut: Shortcut? = ShortcutSettings.command {
        didSet {
            ShortcutSettings.command = commandShortcut
            hotkey.reload()
        }
    }

    /// Accès facultatif au raccourci de dictée, pour l'enregistreur commun.
    var dictateShortcutSetting: Shortcut? {
        get { dictateShortcut }
        set { if let newValue { dictateShortcut = newValue } }
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

    /// Retrouve la même langue dans la liste du moteur courant : « fr-FR »
    /// devient « fr » avec Whisper, et inversement.
    private func equivalentLocale(_ localeID: String) -> String? {
        let code = Locale(identifier: localeID).language.languageCode?.identifier
        return availableLocaleIDs.first {
            Locale(identifier: $0).language.languageCode?.identifier == code
        }
    }

    /// Nom d'une langue, dans la langue de l'interface.
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

    /// Modèle en cours de téléchargement ou de chargement. N'empêche pas de
    /// dicter : l'audio attend le moteur (voir `EngineFeed`).
    @Published var isPreparingModel = false

    let hotkey = HotkeyManager()
    /// Polissage sur l'appareil, par le modèle du système.
    let polisher = FoundationModelsPolisher()
    /// Dictée en cours, du déclenchement à l'insertion.
    var session: DictationSession?
    /// Dernière insertion, pour pouvoir l'annuler.
    @Published var lastInsertion: InsertionRecord?
    /// Message bref affiché par la pill (erreur, annulation…). Les erreurs
    /// restent aussi dans `lastError`, lisible dans les réglages.
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

        // Réévaluer quand l'utilisateur revient des Réglages Système.
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
            // Une combinaison (Fn + ↑) n'annule qu'un enregistrement ; pendant
            // le traitement, elle concerne l'app, pas la dictée précédente.
            if reason == .chord, self.phase != .recording { return }
            self.cancelDictation()
        }
        hotkey.start()

        applyRetention()
        scheduleDailyMaintenance()

        let supported = await TranscriptionSession.supportedLocales()
            .map { $0.identifier(.bcp47) }
        appleLocaleIDs = supported

        // Charge le moteur choisi pour ne pas payer l'attente à la première
        // dictée.
        await preloadEngine()

        await UpdateChecker.shared.checkIfDue()
    }
}
