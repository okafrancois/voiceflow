import AppKit
import AVFoundation

/// Brief message shown by the pill.
struct Notice: Equatable {
    enum Kind { case info, error }
    let id = UUID()
    let text: String
    let kind: Kind
}

/// A dictation, from trigger to insertion.
///
/// The mic starts right away; the engine and insertion target prepare
/// in parallel. A cancelled dictation keeps its place until its tasks
/// finish, but no longer touches the app's state.
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

/// What's needed to undo the last insertion.
struct InsertionRecord {
    let text: String
    let method: InjectionMethod
    let bundleID: String?
    /// Field and position, when the insertion went through accessibility.
    let element: AccessibilityTarget?
    let location: Int?
    /// Selected text that the insertion replaced, to be restored.
    let replaced: String?
}

extension AppState {
    // MARK: - Triggering

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
            // Never asked (onboarding skipped): ask the question now. No
            // automatic start afterward: the key may have been released
            // while the system dialog was up.
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
            // Command mode produces the final text itself.
            polish: kind == .dictate && polishEnabled,
            engineChoice: engineChoice,
            localeID: VocabularyStore.shared.localeID(forBundleID: bundleID) ?? dictationLocaleID,
            bundleID: bundleID,
            appName: frontmost?.localizedName)
        self.session = session

        // Immediate feedback first: pill and sound fire with the key press.
        phase = .recording
        playSound(start: true)
        Diagnostics.log(
            "\(kind == .command ? "command" : "dictation") started · engine \(session.engineChoice.rawValue) · "
            + "language \(session.localeID) · polish \(session.polish)")

        // Mic before engine: whatever is said while the engine prepares
        // waits in `feed` instead of being lost.
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

        // Command mode needs the field and its selection, regardless of
        // the insertion setting.
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
        // Identifier rather than reference: the engine keeps this callback,
        // and the dictation keeps the engine.
        let sessionID = ObjectIdentifier(session)
        session.engineTask = Task { [weak self] in
            guard let self else { throw CancellationError() }
            let engine = try await self.makeEngine(
                choice: session.engineChoice, localeID: session.localeID, hints: hints
            ) { [weak self] volatile in
                Task { @MainActor in
                    // A cancelled dictation must not write into the next
                    // one's pill.
                    guard let self, self.session.map(ObjectIdentifier.init) == sessionID else { return }
                    self.volatileTranscript = volatile
                }
            }
            session.feed.attach(engine)
            return engine
        }
        // An engine that fails to start must be noticed right away, not
        // when the key is released.
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

    /// A model that needs downloading doesn't download silently during
    /// a dictation: it is announced, and the download starts.
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

    /// Model download whose failure is announced, never hidden.
    func download(_ work: @escaping () async throws -> URL) async {
        do {
            _ = try await work()
            await preloadEngine()
        } catch {
            report(L.t("Téléchargement du modèle échoué") + " : \(error.localizedDescription)")
            log.error("model download failed: \(error)")
        }
    }

    // MARK: - Stopping

    func stopDictation() async {
        guard phase == .recording, let session else { return }
        phase = .transcribing
        playSound(start: false)
        let peak = session.recorder.peakLevel
        session.recorder.stop()
        resetMeters()
        let audioDurationMs = Int(-session.startedAt.timeIntervalSinceNow * 1000)

        // Absolute silence: no point asking the engine, and above all it
        // must be reported — macOS doesn't signal a missing mic permission,
        // it just delivers empty buffers.
        Diagnostics.log("recording finished · \(audioDurationMs) ms · peak \(String(format: "%.3f", peak))")
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
            Diagnostics.log("transcription failed: \(error.localizedDescription)")
            report(L.t("Transcription échouée") + " : \(error.localizedDescription)")
            log.error("transcription failed: \(error)")
            return
        }
        if isCurrent(session) { volatileTranscript = "" }

        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            guard !session.cancelled else { return }
            abandon(session)
            // Complete silence rather than failure: say so, otherwise the app
            // looks like it's spinning idly.
            Diagnostics.log("empty transcription (silence trimming: \(trimSilence))")
            report(trimSilence
                ? L.t("Aucune parole reconnue. Si cela se répète, baissez la sensibilité de la coupe du silence, ou désactivez-la.")
                : L.t("Aucune parole reconnue."))
            log.info("empty transcription (trimSilence=\(self.trimSilence))")
            return
        }
        Diagnostics.log("transcribed \(trimmed.count) characters in \(sttDurationMs) ms")

        // Voice commands, then snippets and dictionary, before anything else.
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
            // An app-specific rule overrides the default style.
            let templateID = VocabularyStore.shared.templateID(forBundleID: session.bundleID)
                ?? polishTemplateID
            do {
                final = try await polisher.polish(
                    corrected, template: PolishCatalog.resolved(templateID),
                    locale: session.localeID == Self.autoLocaleID ? nil : session.localeID)
                polishDurationMs = Int(-polishStart.timeIntervalSinceNow * 1000)
                log.info("polished (\(templateID))")
            } catch {
                // Polishing must never cause the dictation to be lost:
                // the raw text is inserted and the failure is reported.
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

        // Cancelled during processing: kept in history, nothing is inserted.
        guard !session.cancelled else {
            Diagnostics.log("dictation cancelled after transcription: kept in history")
            return
        }
        lastTranscript = final
        abandon(session)

        let method = await deliver(final, to: target, localeID: session.localeID)
        if method == .accessibility, let element = target?.accessibility {
            CorrectionWatcher.watch(inserted: lastInsertion?.text ?? final, in: element)
        }
    }

    // MARK: - Command mode

    /// The spoken instruction applies to the selection captured at trigger
    /// time, which it replaces; without a selection, the result is inserted
    /// at the cursor.
    private func runCommand(
        instruction: String, session: DictationSession, target: CapturedTextTarget?
    ) async {
        guard !session.cancelled else { return }
        if isCurrent(session) { phase = .polishing }
        let selection = target?.selection
        Diagnostics.log("command · selection \(selection?.count ?? 0) characters")
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

    // MARK: - Cancellation

    /// Escape during dictation. During recording, everything is discarded;
    /// during processing, the text ends up in history without being
    /// inserted.
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
        Diagnostics.log("dictation cancelled")
    }

    /// The engine of a dictation abandoned during recording must not keep
    /// analyzing into the void.
    private func discardEngine(of session: DictationSession) {
        let task = session.engineTask
        task?.cancel()
        Task.detached {
            if let engine = try? await task?.value { await engine.cancel() }
        }
    }

    /// The dictation no longer drives the app: back to idle if it was the
    /// current dictation.
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

    // MARK: - Engines

    func makeEngine(
        choice: EngineChoice, localeID: String, hints: [String],
        onVolatile: @escaping @Sendable (String) -> Void
    ) async throws -> DictationEngine {
        let isAuto = localeID == Self.autoLocaleID
        let languageCode = isAuto
            ? nil : Locale(identifier: localeID).language.languageCode?.identifier
        if let whisperModel = choice.whisperModel {
            // An English-only model can't detect anything else: letting it
            // guess the language would produce gibberish.
            return try WhisperEngine(
                model: whisperModel,
                language: choice.isEnglishOnly ? "en" : languageCode,
                hints: hints)
        }
        if let sherpa = choice.sherpaModel {
            return try SherpaEngine(model: sherpa, language: languageCode)
        }
        // The system engine requires an explicit language.
        let locale = Locale(identifier: isAuto ? "fr-FR" : localeID)
        return try await TranscriptionSession(locale: locale, hints: hints, onVolatile: onVolatile)
    }

    /// Loads the chosen engine into memory (never a download here).
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

    /// Inserts into the captured field if possible, otherwise into the current focus.
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
                Diagnostics.log("inserted into the original field")
                return .accessibility
            } catch {
                Diagnostics.log("original field unavailable (\(error.localizedDescription)), falling back to current focus")
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
            Diagnostics.log("inserted via \(String(describing: method))")
            return method
        } catch {
            report(L.t("Insertion échouée") + " : \(error.localizedDescription)")
            log.error("injection failed: \(error)")
            return nil
        }
    }

    /// "Paste" targets the field that currently has focus, not the one from
    /// the previous dictation: the user may have switched field or app.
    func reinsertLast() {
        guard !lastTranscript.isEmpty else { return }
        let text = lastTranscript
        Task { await deliver(text, to: nil) }
    }

    func insertEntry(_ entry: HistoryEntry) {
        Task { await deliver(entry.finalText, to: nil) }
    }

    var canUndoLastInsertion: Bool { lastInsertion != nil }

    /// Removes the last insertion and restores what it had replaced.
    /// Via accessibility when the text is still intact in place.
    /// ⌘Z only for a keystroke or paste, in the app that received it:
    /// a write via accessibility doesn't always enter the app's undo
    /// stack, and ⌘Z there would undo something else.
    func undoLastInsertion() {
        guard let record = lastInsertion else { return }
        lastInsertion = nil
        Task {
            if let element = record.element, let location = record.location {
                let restore = record.replaced ?? ""
                if await AXQueue.run({ element.remove(record.text, at: location, restoring: restore) }) {
                    Diagnostics.log("last insertion removed")
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
            Diagnostics.log("last insertion undone via ⌘Z")
        }
    }

    // MARK: - Messages

    /// An error the user must see: in the pill right away, in settings
    /// afterward.
    func report(_ message: String) {
        lastError = message
        notice = Notice(text: message, kind: .error)
        Diagnostics.log("error shown: \(message)")
    }

    // MARK: - Miscellaneous

    /// True if the global keyboard interception is active (Accessibility
    /// permission granted).
    var hotkeyTapActive: Bool { hotkey.isTapActive }

    /// Records the next pressed combination into the given shortcut.
    func captureShortcut(into keyPath: ReferenceWritableKeyPath<AppState, Shortcut?>,
                         completion: @escaping () -> Void) {
        hotkey.beginCapture { [weak self] keyCode, flags in
            Task { @MainActor in
                guard let self else { return }
                defer { completion() }
                // Escape cancels. A single key is only accepted if it isn't used
                // for typing: a modifier (Fn, ⌘…) or a function key.
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

    /// Reloads history off the main thread, then publishes.
    func refreshHistory() {
        Task {
            let snapshot = await HistoryStore.shared.snapshot()
            entries = snapshot.entries
            todayStats = snapshot.today.stats
            hourlyWords = snapshot.today.hourly
            weekUsage = snapshot.week
        }
    }

    /// Retention applies at launch and then regularly: an app that runs
    /// for weeks must not keep more than requested.
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

    /// Requests mic access (system dialog, only once in the app's
    /// lifetime). Called from onboarding or on the first dictation.
    @discardableResult
    func requestMicrophone() async -> Bool {
        let granted = await Permissions.requestMicrophone()
        microphoneGranted = granted
        if !granted {
            report(L.t("Accès micro refusé — Réglages Système › Confidentialité › Microphone"))
        }
        return granted
    }

    /// Shows the system accessibility prompt.
    func requestAccessibility() {
        Permissions.ensureAccessibility()
        refreshPermissions()
    }

    /// Re-evaluates permissions (when the window opens).
    func refreshPermissions() {
        let wasGranted = accessibilityGranted
        accessibilityGranted = Permissions.isAccessibilityTrusted()
        microphoneGranted = Permissions.isMicrophoneGranted()

        // Permission just arrived: set up the interception.
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
