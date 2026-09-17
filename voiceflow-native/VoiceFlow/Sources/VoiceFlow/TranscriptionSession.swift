import AVFoundation
import Speech

/// Une dictée = une session : SpeechAnalyzer + SpeechTranscriber (API macOS 26),
/// entièrement sur l'appareil.
final class TranscriptionSession {
    enum SessionError: LocalizedError {
        case unsupportedLocale
        case noAudioFormat

        var errorDescription: String? {
            switch self {
            case .unsupportedLocale: "Langue non prise en charge par SpeechAnalyzer"
            case .noAudioFormat: "Aucun format audio compatible avec le transcripteur"
            }
        }
    }

    private let transcriber: SpeechTranscriber
    private let analyzer: SpeechAnalyzer
    private let inputStream: AsyncStream<AnalyzerInput>
    private let inputContinuation: AsyncStream<AnalyzerInput>.Continuation
    private let analyzerFormat: AVAudioFormat
    private var converter: AVAudioConverter?
    private var resultsTask: Task<String, Error>?

    /// Locales couvertes par SpeechAnalyzer sur cette machine.
    static func supportedLocales() async -> [Locale] {
        await SpeechTranscriber.supportedLocales
    }

    /// Résout une locale demandée vers la locale supportée correspondante.
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

    /// Télécharge le modèle de la locale demandée si nécessaire.
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

    init(locale requested: Locale, onVolatile: @escaping (String) -> Void) async throws {
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

        try await analyzer.start(inputSequence: inputStream)
    }

    /// Reçoit un buffer au format du micro, le convertit au format de
    /// l'analyseur et le pousse dans la file.
    func feed(_ buffer: AVAudioPCMBuffer) {
        do {
            let converted = try convert(buffer)
            inputContinuation.yield(AnalyzerInput(buffer: converted))
        } catch {
            log.error("audio conversion failed: \(error)")
        }
    }

    /// Clôt le flux, attend la fin de l'analyse et rend le texte final.
    func finish() async throws -> String {
        inputContinuation.finish()
        try await analyzer.finalizeAndFinishThroughEndOfInput()
        guard let resultsTask else { return "" }
        return try await resultsTask.value
    }

    private func convert(_ buffer: AVAudioPCMBuffer) throws -> AVAudioPCMBuffer {
        if buffer.format == analyzerFormat {
            return buffer
        }
        if converter == nil || converter?.inputFormat != buffer.format {
            converter = AVAudioConverter(from: buffer.format, to: analyzerFormat)
            converter?.primeMethod = .none
        }
        guard let converter else {
            throw SessionError.noAudioFormat
        }

        let ratio = analyzerFormat.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 16
        guard let output = AVAudioPCMBuffer(pcmFormat: analyzerFormat, frameCapacity: capacity) else {
            throw SessionError.noAudioFormat
        }

        var fed = false
        var conversionError: NSError?
        converter.convert(to: output, error: &conversionError) { _, status in
            if fed {
                status.pointee = .noDataNow
                return nil
            }
            fed = true
            status.pointee = .haveData
            return buffer
        }
        if let conversionError {
            throw conversionError
        }
        return output
    }
}
