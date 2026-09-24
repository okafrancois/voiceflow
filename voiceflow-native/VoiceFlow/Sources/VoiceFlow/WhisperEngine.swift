import AVFoundation
import WhisperKit

/// Whisper engine via WhisperKit (Core ML, Apple Neural Engine).
/// Accumulates audio as 16 kHz mono Float32 during recording, transcribes
/// in one pass at the end. The model is downloaded from Hugging Face on
/// first use, then cached by WhisperKit.
final class WhisperEngine: DictationEngine, @unchecked Sendable {
    enum EngineError: LocalizedError {
        case emptyRecording

        var errorDescription: String? {
            switch self {
            case .emptyRecording: L.t("Aucun audio capturé")
            }
        }
    }

    private let model: String
    /// Whisper language code ("fr", "en", …); nil = automatic detection.
    private let language: String?
    /// Dictionary terms, passed to the decoder as context.
    private let hints: [String]

    private let resampler: AudioResampler
    private var samples: [Float] = []
    private let lock = NSLock()

    init(model: String, language: String?, hints: [String] = []) throws {
        self.model = model
        self.language = language
        self.hints = hints
        resampler = AudioResampler(to: try AudioResampler.standard16k())
    }

    func feed(_ buffer: AVAudioPCMBuffer) {
        do {
            let converted = try resampler.convert(buffer)
            guard let channel = converted.floatChannelData?[0] else { return }
            let frames = Int(converted.frameLength)
            lock.lock()
            samples.append(contentsOf: UnsafeBufferPointer(start: channel, count: frames))
            lock.unlock()
        } catch {
            log.error("whisper audio conversion failed: \(error)")
        }
    }

    private func snapshotSamples() -> [Float] {
        lock.withLock { samples }
    }

    func finish() async throws -> String {
        let audio = snapshotSamples()
        guard !audio.isEmpty else { throw EngineError.emptyRecording }

        let kit = try await WhisperKitCache.shared.instance(model: model)
        let options = DecodingOptions(
            task: .transcribe,
            language: language,
            detectLanguage: language == nil,
            promptTokens: Self.promptTokens(for: hints, tokenizer: kit.tokenizer)
        )
        let results = try await WhisperKitCache.decoding.run {
            try await kit.transcribe(audioArray: audio, decodeOptions: options)
        }
        let text = results.map(\.text).joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return Self.removingEchoedPrompt(text, hints: hints)
    }
}

extension WhisperEngine {
    /// On near-silent audio, Whisper sometimes echoes back its prompt: the
    /// glossary list then arrives as if it had been spoken.
    static func removingEchoedPrompt(_ text: String, hints: [String]) -> String {
        guard !hints.isEmpty else { return text }
        let prompt = hints.prefix(40).joined(separator: ", ")
        let normalized = text.trimmingCharacters(in: CharacterSet(charactersIn: " ."))
        if normalized.caseInsensitiveCompare(prompt) == .orderedSame { return "" }
        if text.hasPrefix(prompt) {
            // Only the punctuation that followed the prompt, not the
            // dictation's own trailing punctuation.
            return String(text.dropFirst(prompt.count)
                .drop(while: { " .,".contains($0) || $0.isWhitespace }))
        }
        return text
    }

    /// Whisper's initial prompt serves as context for the decoder: a
    /// glossary there makes it write proper nouns and jargon as intended.
    /// Deliberately short — beyond about a hundred tokens, it takes the
    /// audio's place in the decoder's window.
    static func promptTokens(for hints: [String], tokenizer: WhisperTokenizer?) -> [Int]? {
        guard !hints.isEmpty, let tokenizer else { return nil }
        let text = " " + hints.prefix(40).joined(separator: ", ") + "."
        let tokens = tokenizer.encode(text: text)
            .filter { $0 < tokenizer.specialTokens.specialTokenBegin }
        return tokens.isEmpty ? nil : Array(tokens.prefix(120))
    }
}

/// Tracks already-downloaded Whisper models, so they can be shown in
/// settings without querying the network.
@MainActor
final class WhisperModelStore: ObservableObject {
    static let shared = WhisperModelStore()

    /// Variant → local model folder.
    @Published private(set) var folders: [String: String] =
        UserDefaults.standard.dictionary(forKey: "whisperModelFolders") as? [String: String] ?? [:]

    /// Download in progress: variant and progress fraction (0…1).
    @Published var downloading: (variant: String, fraction: Double)?

    /// Root folder for models, next to the history database.
    static var downloadBase: URL {
        let directory = URL.applicationSupportDirectory
            .appending(path: "VoiceFlow").appending(path: "models")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    func isDownloaded(_ variant: String) -> Bool {
        guard let path = folders[variant] else { return false }
        return FileManager.default.fileExists(atPath: path)
    }

    func folder(_ variant: String) -> URL? {
        guard isDownloaded(variant), let path = folders[variant] else { return nil }
        return URL(fileURLWithPath: path)
    }

    func remember(_ variant: String, folder: URL) {
        folders[variant] = folder.path(percentEncoded: false)
        UserDefaults.standard.set(folders, forKey: "whisperModelFolders")
    }

    func forget(_ variant: String) {
        if let path = folders[variant] {
            try? FileManager.default.removeItem(atPath: path)
        }
        folders[variant] = nil
        UserDefaults.standard.set(folders, forKey: "whisperModelFolders")
    }

    /// Downloads the model if missing, publishing progress.
    @discardableResult
    func ensureAvailable(_ variant: String) async throws -> URL {
        if let folder = folder(variant) { return folder }

        downloading = (variant, 0)
        defer { downloading = nil }
        log.info("downloading Whisper model \(variant)…")
        let folder = try await WhisperKit.download(
            variant: variant,
            downloadBase: Self.downloadBase
        ) { @Sendable progress in
            let fraction = progress.fractionCompleted
            Task { @MainActor in
                WhisperModelStore.shared.downloading = (variant, fraction)
            }
        }
        remember(variant, folder: folder)
        log.info("Whisper model \(variant) ready")
        return folder
    }
}

/// The pipeline is never used by two decodings at once: they go through
/// `WhisperKitCache.decoding` one at a time.
extension WhisperKit: @retroactive @unchecked Sendable {}

/// Keeps the WhisperKit pipeline loaded: loading a model takes several
/// seconds, we only pay that cost once per session. Only one model stays
/// in memory — a Large v3 alone weighs around 3 GB.
actor WhisperKitCache {
    static let shared = WhisperKitCache()
    /// One decoding at a time on the shared pipeline.
    static let decoding = SerialWork()
    private var instances: [String: WhisperKit] = [:]
    private var loading: [String: Task<WhisperKit, Error>] = [:]

    func instance(model: String) async throws -> WhisperKit {
        if let instance = instances[model] { return instance }
        if let task = loading[model] { return try await task.value }

        let task = Task<WhisperKit, Error> {
            // Explicit download: it's what feeds the settings progress
            // bar.
            let folder = try await WhisperModelStore.shared.ensureAvailable(model)
            log.info("loading WhisperKit model \(model)…")
            let config = WhisperKitConfig(
                model: model, modelFolder: folder.path(percentEncoded: false), download: false)
            let kit = try await WhisperKit(config)
            log.info("WhisperKit model \(model) ready")
            return kit
        }
        loading[model] = task
        defer { loading[model] = nil }
        let kit = try await task.value
        // Switching models releases the previous one; a dictation still
        // using it keeps its own reference until it's done.
        instances = [model: kit]
        return kit
    }

    func preload(model: String) async throws {
        _ = try await instance(model: model)
    }
}
