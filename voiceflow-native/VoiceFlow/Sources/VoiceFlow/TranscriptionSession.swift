import AVFoundation
import Speech

/// One dictation = one session: SpeechAnalyzer + SpeechTranscriber
/// (macOS 26 API), entirely on-device.
final class TranscriptionSession: @unchecked Sendable {
    enum SessionError: LocalizedError {
        case unsupportedLocale
        case noAudioFormat

        var errorDescription: String? {
            switch self {
            case .unsupportedLocale: L.t("Langue non prise en charge par SpeechAnalyzer")
            case .noAudioFormat: L.t("Aucun format audio compatible avec le transcripteur")
            }
        }
    }

    private let transcriber: SpeechTranscriber
    private let analyzer: SpeechAnalyzer
    private let inputStream: AsyncStream<AnalyzerInput>
    private let inputContinuation: AsyncStream<AnalyzerInput>.Continuation
    private let analyzerFormat: AVAudioFormat
    private let resampler: AudioResampler
    private var resultsTask: Task<String, Error>?
    private var loggedFormats = false
    private var fedFrames = 0
    private var emptyConversions = 0
    private var conversionErrors = 0

    /// Locales covered by SpeechAnalyzer on this machine.
    static func supportedLocales() async -> [Locale] {
        await SpeechTranscriber.supportedLocales
    }

    /// Resolves a requested locale to the corresponding supported locale.
    static func resolve(_ requested: Locale) async throws -> Locale {
        let supported = await SpeechTranscriber.supportedLocales
        if let exact = supported.first(where: {
            $0.identifier(.bcp47) == requested.identifier(.bcp47)
        }) {
            return exact
        }
        if let sameLanguage = supported.first(where: {
            $0.language.languageCode == requested.language.languageCode
        }) {
            return sameLanguage
        }
        throw SessionError.unsupportedLocale
    }

    /// Downloads the requested locale's model if needed.
    static func prepareAssets(for requested: Locale) async throws {
        let locale = try await resolve(requested)
        let transcriber = SpeechTranscriber(
            locale: locale,
            transcriptionOptions: [],
            reportingOptions: [],
            attributeOptions: []
        )
        if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
            log.info("downloading speech model for \(locale.identifier)…")
            try await request.downloadAndInstall()
            log.info("speech model installed")
        }
    }

    init(locale requested: Locale, hints: [String] = [],
         onVolatile: @escaping @Sendable (String) -> Void) async throws {
        let locale = try await Self.resolve(requested)

        transcriber = SpeechTranscriber(
            locale: locale,
            transcriptionOptions: [],
            reportingOptions: [.volatileResults],
            attributeOptions: []
        )

        if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
            try await request.downloadAndInstall()
        }

        guard let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber]) else {
            throw SessionError.noAudioFormat
        }
        analyzerFormat = format
        resampler = AudioResampler(to: format)

        analyzer = SpeechAnalyzer(modules: [transcriber])
        (inputStream, inputContinuation) = AsyncStream.makeStream(of: AnalyzerInput.self)

        let transcriber = self.transcriber
        resultsTask = Task {
            var finalText = ""
            var volatile = ""
            for try await result in transcriber.results {
                let text = String(result.text.characters)
                if result.isFinal {
                    finalText += text
                    volatile = ""
                } else {
                    volatile = text
                }
                onVolatile(finalText + volatile)
            }
            return finalText
        }

        // Dictionary terms: the model favors them when the audio is
        // ambiguous between several spellings.
        if !hints.isEmpty {
            let context = AnalysisContext()
            context.contextualStrings[.general] = hints
            do {
                try await analyzer.setContext(context)
            } catch {
                log.warning("speech context rejected: \(error.localizedDescription)")
            }
        }

        try await analyzer.start(inputSequence: inputStream)
    }

    /// Receives a buffer in the microphone's format, converts it to the
    /// analyzer's format, and pushes it onto the queue.
    func feed(_ buffer: AVAudioPCMBuffer) {
        do {
            if !loggedFormats {
                loggedFormats = true
                Diagnostics.log(
                    "formats · input \(buffer.format.sampleRate) Hz "
                    + "\(buffer.format.channelCount) channel(s) · "
                    + "analyzer \(analyzerFormat.sampleRate) Hz "
                    + "\(analyzerFormat.channelCount) channel(s)")
            }
            let converted = try resampler.convert(buffer)
            guard converted.frameLength > 0 else {
                emptyConversions += 1
                return
            }
            fedFrames += Int(converted.frameLength)
            inputContinuation.yield(AnalyzerInput(buffer: converted))
        } catch {
            conversionErrors += 1
            log.error("audio conversion failed: \(error)")
        }
    }

    /// Dictation cancelled: cut the analysis without waiting for a result.
    func cancel() async {
        inputContinuation.finish()
        await analyzer.cancelAndFinishNow()
        resultsTask?.cancel()
    }

    /// Closes the stream, waits for the analysis to finish, and returns
    /// the final text.
    func finish() async throws -> String {
        Diagnostics.log(
            "Apple engine · \(fedFrames) samples fed · "
            + "\(emptyConversions) empty conversions · \(conversionErrors) errors")
        inputContinuation.finish()
        try await analyzer.finalizeAndFinishThroughEndOfInput()
        guard let resultsTask else { return "" }
        return try await resultsTask.value
    }
}
