import AppKit
import AVFoundation

/// Message bref affiché par la pill.
struct Notice: Equatable {
    enum Kind { case info, error }
    let id = UUID()
    let text: String
    let kind: Kind
}

/// Une dictée, du déclenchement à l'insertion.
///
/// Le micro démarre tout de suite ; le moteur et la cible d'insertion se
/// préparent en parallèle. Une dictée annulée garde sa place jusqu'au bout
/// de ses tâches mais ne touche plus à l'état de l'app.
@MainActor
final class DictationSession {
    let kind: HotkeyManager.Action
    let feed = EngineFeed()
    let recorder = AudioRecorder()
    let startedAt = Date()
    let polish: Bool
    let engineChoice: EngineChoice
    let localeID: String
    let bundleID: String?
    let appName: String?
    var engineTask: Task<DictationEngine, Error>?
    var targetTask: Task<CapturedTextTarget, Never>?
    var cancelled = false

    init(kind: HotkeyManager.Action, polish: Bool, engineChoice: EngineChoice, localeID: String,
         bundleID: String?, appName: String?) {
        self.kind = kind
        self.polish = polish
        self.engineChoice = engineChoice
        self.localeID = localeID
        self.bundleID = bundleID
        self.appName = appName
    }

    func engine() async throws -> DictationEngine {
        guard let engineTask else { throw CancellationError() }
        return try await engineTask.value
    }

    func target() async -> CapturedTextTarget? {
        await targetTask?.value
    }
}

/// Ce qu'il faut savoir pour annuler la dernière insertion.
struct InsertionRecord {
    let text: String
    let method: InjectionMethod
    let bundleID: String?
    /// Champ et position, quand l'insertion est passée par l'accessibilité.
    let element: AccessibilityTarget?
    let location: Int?
    /// Texte sélectionné que l'insertion a remplacé, à remettre en place.
    let replaced: String?
}

extension AppState {
    // MARK: - Déclenchement

    func toggleDictation() {
        if phase == .recording {
            Task { await stopDictation() }
        } else if phase == .idle {
            startDictation()
        }
    }

    func startDictation(kind: HotkeyManager.Action = .dictate) {
        guard phase == .idle else {
            hotkey.startRejected()
            return
        }
        guard microphoneGranted else {
            hotkey.startRejected()
            // Jamais demandé (onboarding passé) : poser la question
            // maintenant. Pas de démarrage automatique ensuite : la touche a
            // pu être relâchée pendant la boîte système.
            Task {
                if await requestMicrophone() {
                    notice = Notice(text: L.t("Micro autorisé : pressez à nouveau le raccourci."), kind: .info)
                }
            }
            return
        }
        guard ensureModelReady() else {
            hotkey.startRejected()
            return
        }
        if kind == .command {
            do {
                try FoundationModelsPolisher.checkAvailability()
            } catch {
                hotkey.startRejected()
                report(L.t("Mode commande indisponible") + " : \(error.localizedDescription)")
                return
            }
        }

        lastError = nil
        notice = nil
        volatileTranscript = ""

        let frontmost = NSWorkspace.shared.frontmostApplication
        let bundleID = frontmost?.bundleIdentifier
        let session = DictationSession(
            kind: kind,
            // Le mode commande produit lui-même le texte final.
            polish: kind == .dictate && polishEnabled,
            engineChoice: engineChoice,
            localeID: VocabularyStore.shared.localeID(forBundleID: bundleID) ?? dictationLocaleID,
            bundleID: bundleID,
            appName: frontmost?.localizedName)
        self.session = session

        // Retour immédiat d'abord : pill et son partent avec la pression.
        phase = .recording
        playSound(start: true)
        Diagnostics.log(
            "\(kind == .command ? "commande" : "dictée") démarrée · moteur \(session.engineChoice.rawValue) · "
            + "langue \(session.localeID) · polissage \(session.polish)")

        // Le micro avant le moteur : ce qui est dit pendant que le moteur se
        // prépare attend dans `feed` au lieu d'être perdu.
        do {
            try session.recorder.start(
                options: AudioRecorder.Options(
                    deviceID: inputDeviceID,
                    noiseReduction: noiseReduction,
                    trimSilence: trimSilence,
                    silenceMargin: Float(silenceMargin)),
                onBuffer: { [feed = session.feed] buffer in feed.push(buffer) },
                onLevel: { [weak self] level in
                    Task { @MainActor in self?.pushLevel(level) }
                })
        } catch {
            abandon(session)
            report(L.t("Démarrage impossible") + " : \(error.localizedDescription)")
            log.error("recorder start failed: \(error)")
            return
        }
        recordingStart = session.startedAt

        // Le mode commande a besoin du champ et de sa sélection, quel que
        // soit le réglage d'insertion.
        let captureField = insertInOriginalField || kind == .command
        session.targetTask = Task {
            await CapturedTextTarget.capture(
                accessibility: captureField, bundleID: bundleID, appName: session.appName,
                readSelection: true)
        }

        if session.polish {
            let template = PolishCatalog.resolved(
                VocabularyStore.shared.templateID(forBundleID: bundleID) ?? polishTemplateID)
            let polisher = polisher
            Task { await polisher.prewarm(template: template) }
        }

        let hints = VocabularyStore.shared.recognitionHints
        // Identifiant plutôt que référence : le moteur garde ce rappel, et
        // la dictée garde le moteur.
        let sessionID = ObjectIdentifier(session)
        session.engineTask = Task { [weak self] in
            guard let self else { throw CancellationError() }
            let engine = try await self.makeEngine(
                choice: session.engineChoice, localeID: session.localeID, hints: hints
            ) { [weak self] volatile in
                Task { @MainActor in
                    // Une dictée annulée ne doit pas écrire dans la pill de
                    // la suivante.
                    guard let self, self.session.map(ObjectIdentifier.init) == sessionID else { return }
                    self.volatileTranscript = volatile
                }
            }
            session.feed.attach(engine)
            return engine
        }
        // Un moteur qui ne démarre pas doit se voir tout de suite, pas au
        // relâchement de la touche.
        Task { [weak self] in
            do {
                _ = try await session.engine()
                log.info("dictation started (\(session.engineChoice.rawValue))")
            } catch {
                guard let self, self.session === session, self.phase == .recording else { return }
                session.recorder.stop()
                self.discardEngine(of: session)
                self.abandon(session)
                self.report(L.t("Démarrage impossible") + " : \(error.localizedDescription)")
                log.error("engine start failed: \(error)")
            }
        }
    }

    /// Un modèle à télécharger ne se télécharge pas en douce pendant une
    /// dictée : on le dit, et on lance le téléchargement.
    private func ensureModelReady() -> Bool {
        if let variant = engineChoice.whisperModel,
           !WhisperModelStore.shared.isDownloaded(variant) {
            report(L.t("Modèle pas encore téléchargé : téléchargement lancé, suivez-le dans les réglages."))
            Task { await download { try await WhisperModelStore.shared.ensureAvailable(variant) } }
            return false
        }
        if let model = engineChoice.sherpaModel,
           !SherpaModelStore.shared.isDownloaded(model) {
            report(L.t("Modèle pas encore téléchargé : téléchargement lancé, suivez-le dans les réglages."))
            Task { await download { try await SherpaModelStore.shared.ensureAvailable(model) } }
            return false
        }
        return true
    }

    /// Téléchargement de modèle dont l'échec est dit, jamais tu.
    func download(_ work: @escaping () async throws -> URL) async {
        do {
            _ = try await work()
            await preloadEngine()
        } catch {
            report(L.t("Téléchargement du modèle échoué") + " : \(error.localizedDescription)")
            log.error("model download failed: \(error)")
        }
    }

    // MARK: - Arrêt

    func stopDictation() async {
        guard phase == .recording, let session else { return }
        phase = .transcribing
        playSound(start: false)
        let peak = session.recorder.peakLevel
        session.recorder.stop()
        resetMeters()
        let audioDurationMs = Int(-session.startedAt.timeIntervalSinceNow * 1000)

        // Silence absolu : inutile d'interroger le moteur, et surtout il faut
        // le dire — macOS ne signale pas une autorisation micro manquante,
        // il livre simplement des blocs vides.
        Diagnostics.log("prise terminée · \(audioDurationMs) ms · crête \(String(format: "%.3f", peak))")
        guard peak > 0.001 else {
            discardEngine(of: session)
            abandon(session)
            microphoneGranted = Permissions.isMicrophoneGranted()
            report(microphoneGranted
                ? L.t("Aucun son capté. Vérifiez que le micro choisi est le bon et qu'il n'est pas coupé.")
                : L.t("Aucun son capté : VoiceFlow n'a pas l'autorisation micro. Réglages Système › Confidentialité › Microphone."))
            log.error("no audio captured (peak 0) — microphone granted: \(self.microphoneGranted)")
            return
        }

        let text: String
        let sttDurationMs: Int
        do {
            let engine = try await session.engine()
            let sttStart = Date()
            text = try await engine.finish()
            sttDurationMs = Int(-sttStart.timeIntervalSinceNow * 1000)
        } catch {
            guard !session.cancelled else { return }
            abandon(session)
            Diagnostics.log("échec transcription : \(error.localizedDescription)")
            report(L.t("Transcription échouée") + " : \(error.localizedDescription)")
            log.error("transcription failed: \(error)")
            return
        }
        if isCurrent(session) { volatileTranscript = "" }

        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            guard !session.cancelled else { return }
            abandon(session)
            // Silence complet plutôt qu'échec : le dire, sinon l'app a
            // l'air de tourner dans le vide.
            Diagnostics.log("transcription vide (coupe du silence : \(trimSilence))")
            report(trimSilence
                ? L.t("Aucune parole reconnue. Si cela se répète, baissez la sensibilité de la coupe du silence, ou désactivez-la.")
                : L.t("Aucune parole reconnue."))
            log.info("empty transcription (trimSilence=\(self.trimSilence))")
            return
        }
        Diagnostics.log("transcrit \(trimmed.count) caractères en \(sttDurationMs) ms")

        // Commandes vocales, puis extraits et dictionnaire, avant tout le reste.
        let target = await session.target()
        if session.kind == .command {
            await runCommand(instruction: VocabularyStore.shared.apply(to: trimmed),
                             session: session, target: target)
            return
        }

        let commanded = voiceCommandsEnabled
            ? VoiceCommands.apply(to: trimmed, localeID: session.localeID) : trimmed
        let corrected = VocabularyStore.shared.apply(to: commanded)

        var final = corrected
        var polishDurationMs: Int?
        if session.polish, !session.cancelled {
            if isCurrent(session) { phase = .polishing }
            let polishStart = Date()
            // Une règle d'application l'emporte sur le style par défaut.
            let templateID = VocabularyStore.shared.templateID(forBundleID: session.bundleID)
                ?? polishTemplateID
            do {
                final = try await polisher.polish(
                    corrected, template: PolishCatalog.resolved(templateID),
                    locale: session.localeID == Self.autoLocaleID ? nil : session.localeID)
                polishDurationMs = Int(-polishStart.timeIntervalSinceNow * 1000)
                log.info("polished (\(templateID))")
            } catch {
                // Le polissage ne doit jamais faire perdre la dictée :
                // on insère le texte brut et on signale l'échec.
                report(L.t("Polissage ignoré, texte brut inséré") + " : \(error.localizedDescription)")
                log.error("polish failed: \(error)")
            }
        }

        HistoryStore.shared.insert(
            rawText: trimmed, finalText: final,
            appName: session.appName,
            engine: session.engineChoice.historyEngine,
            model: session.engineChoice.historyModel,
            language: session.localeID,
            audioDurationMs: audioDurationMs, sttDurationMs: sttDurationMs,
            polishDurationMs: polishDurationMs,
            polishEngine: polishDurationMs == nil ? nil : "foundation-models")
        refreshHistory()

        // Annulée pendant le traitement : gardée dans l'historique, rien
        // n'est inséré.
        guard !session.cancelled else {
            Diagnostics.log("dictée annulée après transcription : conservée dans l'historique")
            return
        }
        lastTranscript = final
        abandon(session)

        let method = await deliver(final, to: target, localeID: session.localeID)
        if method == .accessibility, let element = target?.accessibility {
            CorrectionWatcher.watch(inserted: lastInsertion?.text ?? final, in: element)
        }
    }

    // MARK: - Mode commande

    /// La consigne dite s'applique à la sélection capturée au déclenchement,
    /// qu'elle remplace ; sans sélection, le résultat s'insère au curseur.
    private func runCommand(
        instruction: String, session: DictationSession, target: CapturedTextTarget?
    ) async {
        guard !session.cancelled else { return }
        if isCurrent(session) { phase = .polishing }
        let selection = target?.selection
        Diagnostics.log("commande · sélection \(selection?.count ?? 0) caractères")
        let output: String
        do {
            output = try await polisher.transform(selection: selection, instruction: instruction)
        } catch {
            abandon(session)
            report(L.t("Commande échouée") + " : \(error.localizedDescription)")
            log.error("command failed: \(error)")
            return
        }
        guard !session.cancelled else { return }
        abandon(session)
        lastTranscript = output
        await deliver(output, to: target, shaping: selection == nil)
    }

    // MARK: - Annulation

    /// Échap pendant la dictée. Pendant l'enregistrement, tout est jeté ;
    /// pendant le traitement, le texte finit dans l'historique sans être
    /// inséré.
    func cancelDictation() {
        guard let session, phase != .idle else { return }
        session.cancelled = true
        if phase == .recording {
            session.recorder.stop()
            discardEngine(of: session)
        }
        abandon(session)
        volatileTranscript = ""
        playSound(start: false)
        notice = Notice(text: L.t("Dictée annulée"), kind: .info)
        Diagnostics.log("dictée annulée")
    }

    /// Le moteur d'une dictée abandonnée pendant l'enregistrement ne doit
    /// pas continuer d'analyser dans le vide.
    private func discardEngine(of session: DictationSession) {
        let task = session.engineTask
        task?.cancel()
        Task.detached {
            if let engine = try? await task?.value { await engine.cancel() }
        }
    }

    /// La dictée ne pilote plus l'app : retour au repos si elle était la
    /// dictée courante.
    func abandon(_ session: DictationSession) {
        guard isCurrent(session) else { return }
        self.session = nil
        resetMeters()
        phase = .idle
    }

    func isCurrent(_ session: DictationSession) -> Bool {
        self.session === session
    }

    private func resetMeters() {
        recordingStart = nil
        audioLevels = Array(repeating: 0, count: audioLevels.count)
    }

    // MARK: - Moteurs

    func makeEngine(
        choice: EngineChoice, localeID: String, hints: [String],
        onVolatile: @escaping @Sendable (String) -> Void
    ) async throws -> DictationEngine {
        let isAuto = localeID == Self.autoLocaleID
        let languageCode = isAuto
            ? nil : Locale(identifier: localeID).language.languageCode?.identifier
        if let whisperModel = choice.whisperModel {
            // Un modèle anglais seul ne sait rien détecter d'autre : lui
            // laisser deviner la langue produirait du charabia.
            return try WhisperEngine(
                model: whisperModel,
                language: choice.isEnglishOnly ? "en" : languageCode,
                hints: hints)
        }
        if let sherpa = choice.sherpaModel {
            return try SherpaEngine(model: sherpa, language: languageCode)
        }
        // Le moteur système exige une langue explicite.
        let locale = Locale(identifier: isAuto ? "fr-FR" : localeID)
        return try await TranscriptionSession(locale: locale, hints: hints, onVolatile: onVolatile)
    }

    /// Charge le moteur choisi en mémoire (jamais de téléchargement ici).
    func preloadEngine() async {
        let choice = engineChoice
        let localeID = dictationLocaleID
        preparingCount += 1
        isPreparingModel = true
        defer {
            preparingCount -= 1
            isPreparingModel = preparingCount > 0
        }
        do {
            if let variant = choice.whisperModel {
                guard WhisperModelStore.shared.isDownloaded(variant) else { return }
                try await WhisperKitCache.shared.preload(model: variant)
            } else if let sherpa = choice.sherpaModel {
                guard SherpaModelStore.shared.isDownloaded(sherpa) else { return }
                let code = localeID == Self.autoLocaleID
                    ? nil : Locale(identifier: localeID).language.languageCode?.identifier
                try await SherpaRecognizerCache.shared.preload(model: sherpa, language: code)
            } else if localeID != Self.autoLocaleID {
                try await TranscriptionSession.prepareAssets(for: Locale(identifier: localeID))
            }
        } catch {
            lastError = L.t("Modèle de transcription indisponible") + " : \(error.localizedDescription)"
            log.error("engine preload failed: \(error)")
        }
    }

    // MARK: - Insertion

    /// Insère dans le champ capturé si possible, sinon dans le focus courant.
    @discardableResult
    func deliver(
        _ text: String, to target: CapturedTextTarget?, shaping: Bool = true,
        localeID: String? = nil
    ) async -> InjectionMethod? {
        if let element = target?.accessibility {
            let context = shaping && smartSpacingEnabled
                ? await AXQueue.run { element.textBeforeInsertion() } : nil
            let adjusted = SmartSpacing.adjust(text, after: context, localeID: localeID)
            do {
                let location = try await AXQueue.run { try element.insert(adjusted) }
                lastInsertion = InsertionRecord(
                    text: adjusted, method: .accessibility, bundleID: target?.bundleID,
                    element: element, location: location, replaced: target?.selection)
                Diagnostics.log("inséré dans le champ d'origine")
                return .accessibility
            } catch {
                Diagnostics.log("champ d'origine indisponible (\(error.localizedDescription)), repli sur le focus courant")
            }
        }
        do {
            let method = try await Task.detached(priority: .userInitiated) {
                try TextInjector.insert(text)
            }.value
            lastInsertion = InsertionRecord(
                text: text, method: method,
                bundleID: NSWorkspace.shared.frontmostApplication?.bundleIdentifier,
                element: nil, location: nil, replaced: nil)
            Diagnostics.log("inséré via \(String(describing: method))")
            return method
        } catch {
            report(L.t("Insertion échouée") + " : \(error.localizedDescription)")
            log.error("injection failed: \(error)")
            return nil
        }
    }

    /// « Coller » vise le champ qui a le focus maintenant, pas celui de la
    /// dictée précédente : l'utilisateur a pu changer de champ ou d'app.
    func reinsertLast() {
        guard !lastTranscript.isEmpty else { return }
        let text = lastTranscript
        Task { await deliver(text, to: nil) }
    }

    func insertEntry(_ entry: HistoryEntry) {
        Task { await deliver(entry.finalText, to: nil) }
    }

    var canUndoLastInsertion: Bool { lastInsertion != nil }

    /// Retire la dernière insertion et remet ce qu'elle avait remplacé.
    /// Par l'accessibilité quand le texte est encore intact à sa place.
    /// ⌘Z seulement pour une frappe ou un collage, dans l'app qui l'a reçu :
    /// une écriture par accessibilité n'entre pas toujours dans la pile
    /// d'annulation de l'app, et ⌘Z y déferait autre chose.
    func undoLastInsertion() {
        guard let record = lastInsertion else { return }
        lastInsertion = nil
        Task {
            if let element = record.element, let location = record.location {
                let restore = record.replaced ?? ""
                if await AXQueue.run({ element.remove(record.text, at: location, restoring: restore) }) {
                    Diagnostics.log("dernière insertion retirée")
                    return
                }
            }
            guard record.method != .accessibility else {
                report(L.t("Impossible d'annuler : le texte inséré a été modifié depuis."))
                return
            }
            guard NSWorkspace.shared.frontmostApplication?.bundleIdentifier == record.bundleID else {
                report(L.t("Impossible d'annuler : l'application qui a reçu le texte n'est plus au premier plan."))
                return
            }
            await Task.detached { TextInjector.pressUndo() }.value
            Diagnostics.log("dernière insertion annulée par ⌘Z")
        }
    }

    // MARK: - Messages

    /// Une erreur que l'utilisateur doit voir : dans la pill tout de suite,
    /// dans les réglages ensuite.
    func report(_ message: String) {
        lastError = message
        notice = Notice(text: message, kind: .error)
        Diagnostics.log("erreur affichée : \(message)")
    }

    // MARK: - Divers

    /// Vrai si l'interception clavier globale fonctionne (permission
    /// Accessibilité accordée).
    var hotkeyTapActive: Bool { hotkey.isTapActive }

    /// Enregistre la prochaine combinaison pressée dans le raccourci donné.
    func captureShortcut(into keyPath: ReferenceWritableKeyPath<AppState, Shortcut?>,
                         completion: @escaping () -> Void) {
        hotkey.beginCapture { [weak self] keyCode, flags in
            Task { @MainActor in
                guard let self else { return }
                defer { completion() }
                // Échap annule. Une touche seule n'est acceptée que si elle ne
                // sert pas à écrire : modificateur (Fn, ⌘…) ou touche de fonction.
                guard keyCode != 53 else { return }
                guard Shortcut.isAssignable(keyCode: keyCode, modifiers: flags) else {
                    self.lastError = L.t("Raccourci refusé : une touche ordinaire seule serait avalée partout. Utilisez Fn, une touche de fonction, ou ajoutez un modificateur.")
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

    /// Relit l'historique hors du fil principal, puis publie.
    func refreshHistory() {
        Task {
            let snapshot = await HistoryStore.shared.snapshot()
            entries = snapshot.entries
            todayStats = snapshot.today.stats
            hourlyWords = snapshot.today.hourly
            weekUsage = snapshot.week
        }
    }

    /// La rétention s'applique au lancement puis régulièrement : une app
    /// qui tourne des semaines ne doit pas garder plus que demandé.
    func applyRetention() {
        guard let days = retentionDays else { return }
        HistoryStore.shared.deleteOlderThan(days: days)
    }

    func scheduleDailyMaintenance() {
        maintenanceTask?.cancel()
        maintenanceTask = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(6 * 3600))
                guard let self, !Task.isCancelled else { return }
                self.applyRetention()
                self.refreshHistory()
                await UpdateChecker.shared.checkIfDue()
            }
        }
    }

    /// Demande l'accès micro (boîte système, une seule fois dans la vie de
    /// l'app). Appelé depuis l'onboarding ou à la première dictée.
    @discardableResult
    func requestMicrophone() async -> Bool {
        let granted = await Permissions.requestMicrophone()
        microphoneGranted = granted
        if !granted {
            report(L.t("Accès micro refusé — Réglages Système › Confidentialité › Microphone"))
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

    func playSound(start: Bool) {
        guard soundsEnabled else { return }
        BeepPlayer.shared.play(start: start)
    }

    func pushLevel(_ level: Float) {
        audioLevels.removeFirst()
        audioLevels.append(level)
    }
}
