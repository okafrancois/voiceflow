import AVFoundation
import Foundation
import SherpaOnnx
import SherpaOnnxC

/// The ONNX models run by sherpa-onnx: SenseVoice and Qwen3-ASR, the two
/// non-Whisper engines from the Tauri app.
///
/// Files are downloaded one by one from Hugging Face rather than as an
/// archive: nothing to decompress, so no external process to launch from
/// a hardened-runtime app.
struct SherpaModel: Identifiable, Equatable {
    enum Kind: Equatable {
        /// A single model file, plus a tokens file.
        case senseVoice
        /// Convolutional frontend, encoder, decoder, and a tokenizer
        /// folder.
        case qwen3ASR
    }

    let id: String
    let kind: Kind
    /// Hugging Face repo the files come from.
    let repo: String
    /// Relative paths to download, both in the repo and on disk.
    let files: [String]

    static let senseVoice = SherpaModel(
        id: "sense-voice-small",
        kind: .senseVoice,
        repo: "csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
        files: ["model.int8.onnx", "tokens.txt"])

    static let qwen3ASR = SherpaModel(
        id: "qwen3-asr-0.6b-int8",
        kind: .qwen3ASR,
        repo: "pantinor/sherpa-onnx-qwen3-asr-0.6b-int8",
        files: [
            "conv_frontend.onnx",
            "encoder.int8.onnx",
            "decoder.int8.onnx",
            "tokenizer/vocab.json",
            "tokenizer/merges.txt",
            "tokenizer/tokenizer_config.json",
        ])

    static let all: [SherpaModel] = [.senseVoice, .qwen3ASR]

    /// Languages the model can genuinely transcribe, as BCP-47 codes.
    /// Offering the rest would silently give a wrong output.
    var supportedLanguages: [String] {
        switch kind {
        case .senseVoice: ["zh", "yue", "ja", "ko", "en"]
        case .qwen3ASR: ["zh", "en", "fr", "es", "de", "it", "pt", "ru", "ja", "ko", "ar"]
        }
    }

    func url(for file: String) -> URL? {
        URL(string: "https://huggingface.co/\(repo)/resolve/main/\(file)")
    }
}

/// sherpa-onnx engine. Accumulates audio as 16 kHz mono Float32 during
/// recording, decodes in one pass at the end — like Whisper, and unlike
/// Apple's engine, which works continuously.
final class SherpaEngine: DictationEngine, @unchecked Sendable {
    enum EngineError: LocalizedError {
        case emptyRecording

        var errorDescription: String? {
            switch self {
            case .emptyRecording: L.t("Aucun audio capturé")
            }
        }
    }

    private let model: SherpaModel
    /// Model language code ("zh", "en", …); empty = automatic detection.
    private let language: String
    private let resampler: AudioResampler
    private var samples: [Float] = []
    private let lock = NSLock()

    init(model: SherpaModel, language: String?) throws {
        self.model = model
        self.language = Self.languageKey(model: model, language: language)
        resampler = AudioResampler(to: try AudioResampler.standard16k())
    }

    /// SenseVoice accepts a language hint; Qwen3-ASR detects on its own.
    static func languageKey(model: SherpaModel, language: String?) -> String {
        model.kind == .senseVoice ? (language ?? "") : ""
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
            log.error("sherpa audio conversion failed: \(error)")
        }
    }

    func finish() async throws -> String {
        let audio = lock.withLock { samples }
        Diagnostics.log("engine \(model.id) · \(audio.count) samples fed")
        guard !audio.isEmpty else { throw EngineError.emptyRecording }

        let recognizer = try await SherpaRecognizerCache.shared.recognizer(
            model: model, language: language)

        // Decoding is blocking and heavy: it must not run on the main
        // actor, or the interface would freeze for the duration.
        return try await SherpaRecognizerCache.decoding.run {
            await Task.detached(priority: .userInitiated) {
                let result = recognizer.decode(samples: audio, sampleRate: 16000)
                return result.text.trimmingCharacters(in: .whitespacesAndNewlines)
            }.value
        }
    }
}

/// Only one decoding at a time, via `SherpaRecognizerCache.decoding`.
extension SherpaOnnxOfflineRecognizer: @retroactive @unchecked Sendable {}

/// Keeps recognizers loaded: building Qwen3-ASR's takes several seconds,
/// we only pay that cost once per model and per session.
actor SherpaRecognizerCache {
    static let shared = SherpaRecognizerCache()
    /// One decoding at a time on the shared recognizer.
    static let decoding = SerialWork()

    private var instances: [String: SherpaOnnxOfflineRecognizer] = [:]
    private var loading: [String: Task<SherpaOnnxOfflineRecognizer, Error>] = [:]

    func recognizer(
        model: SherpaModel, language: String
    ) async throws -> SherpaOnnxOfflineRecognizer {
        // Language is part of the key: it's baked into the config.
        let key = "\(model.id)|\(language)"
        if let instance = instances[key] { return instance }
        if let task = loading[key] { return try await task.value }

        let task = Task<SherpaOnnxOfflineRecognizer, Error> {
            let folder = try await SherpaModelStore.shared.ensureAvailable(model)
            log.info("loading sherpa-onnx model \(model.id)…")
            let recognizer = await Task.detached(priority: .userInitiated) {
                Self.build(model: model, language: language, folder: folder)
            }.value
            log.info("sherpa-onnx model \(model.id) ready")
            return recognizer
        }
        loading[key] = task
        defer { loading[key] = nil }
        let recognizer = try await task.value
        // Only one recognizer in memory, as with Whisper.
        instances = [key: recognizer]
        return recognizer
    }

    func preload(model: SherpaModel, language: String?) async throws {
        _ = try await recognizer(
            model: model, language: SherpaEngine.languageKey(model: model, language: language))
    }

    /// CPU decoding benefits from performance cores; two threads left
    /// most of them idle.
    private static var threadCount: Int {
        min(6, max(2, ProcessInfo.processInfo.activeProcessorCount / 2))
    }

    private static func build(
        model: SherpaModel, language: String, folder: URL
    ) -> SherpaOnnxOfflineRecognizer {
        func path(_ file: String) -> String {
            folder.appending(path: file).path(percentEncoded: false)
        }

        let modelConfig: SherpaOnnxOfflineModelConfig
        switch model.kind {
        case .senseVoice:
            modelConfig = sherpaOnnxOfflineModelConfig(
                tokens: path("tokens.txt"),
                numThreads: Self.threadCount,
                provider: "cpu",
                senseVoice: sherpaOnnxOfflineSenseVoiceModelConfig(
                    model: path("model.int8.onnx"),
                    language: language,
                    useInverseTextNormalization: true))

        case .qwen3ASR:
            // Qwen3-ASR doesn't use a tokens file: its vocabulary lives
            // in the tokenizer folder.
            modelConfig = sherpaOnnxOfflineModelConfig(
                tokens: "",
                numThreads: Self.threadCount,
                provider: "cpu",
                qwen3Asr: sherpaOnnxOfflineQwen3ASRModelConfig(
                    convFrontend: path("conv_frontend.onnx"),
                    encoder: path("encoder.int8.onnx"),
                    decoder: path("decoder.int8.onnx"),
                    tokenizer: path("tokenizer")))
        }

        var config = sherpaOnnxOfflineRecognizerConfig(
            featConfig: sherpaOnnxFeatureConfig(sampleRate: 16000, featureDim: 80),
            modelConfig: modelConfig)
        return SherpaOnnxOfflineRecognizer(config: &config)
    }
}

/// Download and tracking of sherpa-onnx models, mirroring
/// `WhisperModelStore` for Core ML models.
@MainActor
final class SherpaModelStore: ObservableObject {
    static let shared = SherpaModelStore()

    enum StoreError: LocalizedError {
        case badURL(String)
        case badResponse(String)

        var errorDescription: String? {
            switch self {
            case .badURL(let file): L.t("Adresse de téléchargement invalide") + " : \(file)"
            case .badResponse(let file): L.t("Téléchargement refusé par le serveur") + " : \(file)"
            }
        }
    }

    /// Model identifier → local folder.
    @Published private(set) var folders: [String: String] =
        UserDefaults.standard.dictionary(forKey: "sherpaModelFolders") as? [String: String] ?? [:]

    /// Download in progress: model and progress fraction (0…1).
    @Published var downloading: (model: String, fraction: Double)?

    private static var base: URL {
        let directory = URL.applicationSupportDirectory
            .appending(path: "VoiceFlow").appending(path: "models").appending(path: "sherpa")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    /// Present means: all files are on disk. We check the default
    /// location even if settings no longer remember it, so as not to
    /// re-download a gigabyte that's already there.
    func isDownloaded(_ model: SherpaModel) -> Bool {
        let candidate = folders[model.id].map(URL.init(fileURLWithPath:))
            ?? Self.base.appending(path: model.id)
        return model.files.allSatisfy {
            FileManager.default.fileExists(atPath: candidate.appending(path: $0).path)
        }
    }

    func forget(_ model: SherpaModel) {
        if let path = folders[model.id] {
            try? FileManager.default.removeItem(atPath: path)
        }
        folders[model.id] = nil
        UserDefaults.standard.set(folders, forKey: "sherpaModelFolders")
    }

    /// Downloads the model if missing, publishing progress.
    @discardableResult
    func ensureAvailable(_ model: SherpaModel) async throws -> URL {
        let destination = Self.base.appending(path: model.id)
        if isDownloaded(model) { return destination }

        downloading = (model.id, 0)
        defer { downloading = nil }
        log.info("downloading sherpa-onnx model \(model.id)…")

        // Download into a temporary folder: an interruption must not
        // leave behind a half-written model that would later pass for
        // complete.
        let staging = Self.base.appending(path: model.id + ".partial")
        try? FileManager.default.removeItem(at: staging)
        try FileManager.default.createDirectory(at: staging, withIntermediateDirectories: true)

        let sizes = await Self.sizes(of: model)
        let total = sizes.values.reduce(0, +)
        var done: Int64 = 0

        for file in model.files {
            guard let url = model.url(for: file) else { throw StoreError.badURL(file) }
            let target = staging.appending(path: file)
            try FileManager.default.createDirectory(
                at: target.deletingLastPathComponent(), withIntermediateDirectories: true)

            let completed = done
            let progress = DownloadProgress { written in
                guard total > 0 else { return }
                let fraction = Double(completed + written) / Double(total)
                Task { @MainActor in
                    SherpaModelStore.shared.downloading = (model.id, min(1, fraction))
                }
            }
            let (temporary, response) = try await URLSession.shared.download(
                from: url, delegate: progress)
            guard (response as? HTTPURLResponse)?.statusCode == 200 else {
                throw StoreError.badResponse(file)
            }
            try FileManager.default.moveItem(at: temporary, to: target)
            done += sizes[file] ?? Self.fileSize(target)
        }

        try? FileManager.default.removeItem(at: destination)
        try FileManager.default.moveItem(at: staging, to: destination)

        folders[model.id] = destination.path(percentEncoded: false)
        UserDefaults.standard.set(folders, forKey: "sherpaModelFolders")
        log.info("sherpa-onnx model \(model.id) ready")
        return destination
    }

    private static func fileSize(_ url: URL) -> Int64 {
        (try? FileManager.default.attributesOfItem(atPath: url.path))?[.size] as? Int64 ?? 0
    }

    /// Size of each file, for an honest progress bar: Qwen3-ASR's decoder
    /// alone accounts for three quarters of the model's weight.
    private static func sizes(of model: SherpaModel) async -> [String: Int64] {
        var sizes: [String: Int64] = [:]
        for file in model.files {
            guard let url = model.url(for: file) else { continue }
            var request = URLRequest(url: url)
            request.httpMethod = "HEAD"
            guard let (_, response) = try? await URLSession.shared.data(for: request),
                  response.expectedContentLength > 0
            else { continue }
            sizes[file] = response.expectedContentLength
        }
        return sizes
    }
}

/// Relays a download's progress: URLSession's async API hands back the
/// file all at once, without saying anything about the path taken.
private final class DownloadProgress: NSObject, URLSessionDownloadDelegate {
    private let onProgress: @Sendable (Int64) -> Void

    init(onProgress: @escaping @Sendable (Int64) -> Void) {
        self.onProgress = onProgress
    }

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64,
        totalBytesExpectedToWrite: Int64
    ) {
        onProgress(totalBytesWritten)
    }

    /// Required by the protocol; the async API retrieves the file
    /// itself, there's nothing to do here.
    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didFinishDownloadingTo location: URL
    ) {}
}
