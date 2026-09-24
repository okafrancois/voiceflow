import AVFoundation
import WhisperKit

/// Moteur Whisper via WhisperKit (Core ML, Apple Neural Engine).
/// Accumule l'audio en 16 kHz mono Float32 pendant l'enregistrement,
/// transcrit en une passe à la fin. Le modèle est téléchargé depuis
/// Hugging Face au premier usage puis mis en cache par WhisperKit.
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
    /// Code langue Whisper ("fr", "en", …) ; nil = détection automatique.
    private let language: String?
    /// Termes du dictionnaire, passés au décodeur comme contexte.
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
    /// Sur un audio presque muet, Whisper recrache parfois son prompt : la
    /// liste du glossaire arrive alors comme si elle avait été dite.
    static func removingEchoedPrompt(_ text: String, hints: [String]) -> String {
        guard !hints.isEmpty else { return text }
        let prompt = hints.prefix(40).joined(separator: ", ")
        let normalized = text.trimmingCharacters(in: CharacterSet(charactersIn: " ."))
        if normalized.caseInsensitiveCompare(prompt) == .orderedSame { return "" }
        if text.hasPrefix(prompt) {
            // Seulement la ponctuation qui suivait le prompt, pas celle de
            // la fin de la dictée.
            return String(text.dropFirst(prompt.count)
                .drop(while: { " .,".contains($0) || $0.isWhitespace }))
        }
        return text
    }

    /// Le prompt initial de Whisper sert de contexte au décodeur : un
    /// glossaire y fait écrire les noms propres et le jargon comme voulu.
    /// Court exprès — au-delà d'une centaine de jetons, il prend la place
    /// de l'audio dans la fenêtre du décodeur.
    static func promptTokens(for hints: [String], tokenizer: WhisperTokenizer?) -> [Int]? {
        guard !hints.isEmpty, let tokenizer else { return nil }
        let text = " " + hints.prefix(40).joined(separator: ", ") + "."
        let tokens = tokenizer.encode(text: text)
            .filter { $0 < tokenizer.specialTokens.specialTokenBegin }
        return tokens.isEmpty ? nil : Array(tokens.prefix(120))
    }
}

/// Suit les modèles Whisper déjà téléchargés, pour pouvoir l'afficher dans
/// les réglages sans interroger le réseau.
@MainActor
final class WhisperModelStore: ObservableObject {
    static let shared = WhisperModelStore()

    /// Variante → dossier local du modèle.
    @Published private(set) var folders: [String: String] =
        UserDefaults.standard.dictionary(forKey: "whisperModelFolders") as? [String: String] ?? [:]

    /// Téléchargement en cours : variante et avancement (0…1).
    @Published var downloading: (variant: String, fraction: Double)?

    /// Dossier racine des modèles, à côté de la base d'historique.
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

    /// Télécharge le modèle s'il manque, en publiant l'avancement.
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

/// Le pipeline n'est jamais utilisé par deux décodages à la fois : ils
/// passent un par un par `WhisperKitCache.decoding`.
extension WhisperKit: @retroactive @unchecked Sendable {}

/// Garde le pipeline WhisperKit chargé : le chargement d'un modèle prend
/// plusieurs secondes, on ne le paie qu'une fois par session. Un seul modèle
/// reste en mémoire — un Large v3 pèse à lui seul autour de 3 Go.
actor WhisperKitCache {
    static let shared = WhisperKitCache()
    /// Un décodage à la fois sur le pipeline partagé.
    static let decoding = SerialWork()
    private var instances: [String: WhisperKit] = [:]
    private var loading: [String: Task<WhisperKit, Error>] = [:]

    func instance(model: String) async throws -> WhisperKit {
        if let instance = instances[model] { return instance }
        if let task = loading[model] { return try await task.value }

        let task = Task<WhisperKit, Error> {
            // Téléchargement explicite : c'est lui qui alimente la barre de
            // progression des réglages.
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
        // Changer de modèle libère le précédent ; une dictée qui l'utilise
        // encore garde sa propre référence jusqu'au bout.
        instances = [model: kit]
        return kit
    }

    func preload(model: String) async throws {
        _ = try await instance(model: model)
    }
}
