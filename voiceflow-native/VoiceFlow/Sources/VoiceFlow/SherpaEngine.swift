import AVFoundation
import Foundation
import SherpaOnnx
import SherpaOnnxC

/// Les modèles ONNX exécutés par sherpa-onnx : SenseVoice et Qwen3-ASR,
/// les deux moteurs non-Whisper de l'app Tauri.
///
/// Les fichiers sont téléchargés un par un depuis Hugging Face plutôt que
/// sous forme d'archive : rien à décompresser, donc aucun processus externe
/// à lancer depuis une app au durcissement d'exécution.
struct SherpaModel: Identifiable, Equatable {
    enum Kind: Equatable {
        /// Un seul fichier de modèle, plus un fichier de jetons.
        case senseVoice
        /// Frontend convolutif, encodeur, décodeur, et un dossier tokenizer.
        case qwen3ASR
    }

    let id: String
    let kind: Kind
    /// Dépôt Hugging Face d'où viennent les fichiers.
    let repo: String
    /// Chemins relatifs à télécharger, dans le dépôt comme sur le disque.
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

    /// Langues que le modèle sait vraiment transcrire, en codes BCP-47.
    /// Proposer le reste donnerait une sortie fausse sans le dire.
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

/// Moteur sherpa-onnx. Accumule l'audio en 16 kHz mono Float32 pendant
/// l'enregistrement, décode en une passe à la fin — comme Whisper, et
/// contrairement au moteur d'Apple qui travaille en continu.
final class SherpaEngine: DictationEngine {
    enum EngineError: LocalizedError {
        case emptyRecording

        var errorDescription: String? {
            switch self {
            case .emptyRecording: "Aucun audio capturé"
            }
        }
    }

    private let model: SherpaModel
    /// Code langue du modèle ("zh", "en", …) ; vide = détection automatique.
    private let language: String
    private let resampler: AudioResampler
    private var samples: [Float] = []
    private let lock = NSLock()

    init(model: SherpaModel, language: String?) throws {
        self.model = model
        // SenseVoice accepte un indice de langue ; Qwen3-ASR détecte seul.
        self.language = model.kind == .senseVoice ? (language ?? "") : ""
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
            log.error("sherpa audio conversion failed: \(error)")
        }
    }

    func finish() async throws -> String {
        let audio = lock.withLock { samples }
        Diagnostics.log("moteur \(model.id) · \(audio.count) échantillons transmis")
        guard !audio.isEmpty else { throw EngineError.emptyRecording }

        let recognizer = try await SherpaRecognizerCache.shared.recognizer(
            model: model, language: language)

        // Le décodage est bloquant et gourmand : il ne doit pas s'exécuter
        // sur l'acteur principal, sinon l'interface se fige le temps du calcul.
        return try await Task.detached(priority: .userInitiated) {
            let result = recognizer.decode(samples: audio, sampleRate: 16000)
            return result.text.trimmingCharacters(in: .whitespacesAndNewlines)
        }.value
    }
}

/// Garde les reconnaisseurs chargés : construire celui de Qwen3-ASR prend
/// plusieurs secondes, on ne le paie qu'une fois par modèle et par session.
actor SherpaRecognizerCache {
    static let shared = SherpaRecognizerCache()

    private var instances: [String: SherpaOnnxOfflineRecognizer] = [:]
    private var loading: [String: Task<SherpaOnnxOfflineRecognizer, Error>] = [:]

    func recognizer(
        model: SherpaModel, language: String
    ) async throws -> SherpaOnnxOfflineRecognizer {
        // La langue fait partie de la clé : elle est figée dans la config.
        let key = "\(model.id)|\(language)"
        if let instance = instances[key] { return instance }
        if let task = loading[key] { return try await task.value }

        let task = Task<SherpaOnnxOfflineRecognizer, Error> {
            let folder = try await SherpaModelStore.shared.ensureAvailable(model)
            log.info("loading sherpa-onnx model \(model.id)…")
            let recognizer = try await Task.detached(priority: .userInitiated) {
                Self.build(model: model, language: language, folder: folder)
            }.value
            log.info("sherpa-onnx model \(model.id) ready")
            return recognizer
        }
        loading[key] = task
        defer { loading[key] = nil }
        let recognizer = try await task.value
        instances[key] = recognizer
        return recognizer
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
                numThreads: 2,
                provider: "cpu",
                senseVoice: sherpaOnnxOfflineSenseVoiceModelConfig(
                    model: path("model.int8.onnx"),
                    language: language,
                    useInverseTextNormalization: true))

        case .qwen3ASR:
            // Qwen3-ASR n'utilise pas de fichier de jetons : son vocabulaire
            // vit dans le dossier tokenizer.
            modelConfig = sherpaOnnxOfflineModelConfig(
                tokens: "",
                numThreads: 2,
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

/// Téléchargement et suivi des modèles sherpa-onnx, à l'image de
/// `WhisperModelStore` pour les modèles Core ML.
@MainActor
final class SherpaModelStore: ObservableObject {
    static let shared = SherpaModelStore()

    enum StoreError: LocalizedError {
        case badURL(String)
        case badResponse(String)

        var errorDescription: String? {
            switch self {
            case .badURL(let file): "Adresse de téléchargement invalide : \(file)"
            case .badResponse(let file): "Téléchargement refusé par le serveur : \(file)"
            }
        }
    }

    /// Identifiant de modèle → dossier local.
    @Published private(set) var folders: [String: String] =
        UserDefaults.standard.dictionary(forKey: "sherpaModelFolders") as? [String: String] ?? [:]

    /// Téléchargement en cours : modèle et avancement (0…1).
    @Published var downloading: (model: String, fraction: Double)?

    private static var base: URL {
        let directory = URL.applicationSupportDirectory
            .appending(path: "VoiceFlow").appending(path: "models").appending(path: "sherpa")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    /// Présent signifie : tous les fichiers sont sur le disque. On regarde
    /// l'emplacement par défaut même si les réglages ne s'en souviennent
    /// plus, pour ne pas retélécharger un gigaoctet déjà là.
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

    /// Télécharge le modèle s'il manque, en publiant l'avancement.
    @discardableResult
    func ensureAvailable(_ model: SherpaModel) async throws -> URL {
        let destination = Self.base.appending(path: model.id)
        if isDownloaded(model) { return destination }

        downloading = (model.id, 0)
        defer { downloading = nil }
        log.info("downloading sherpa-onnx model \(model.id)…")

        // Téléchargement dans un dossier temporaire : une interruption ne
        // doit pas laisser derrière elle un modèle à moitié écrit qui
        // passerait ensuite pour complet.
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
        (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int64) as? Int64 ?? 0
    }

    /// Taille de chaque fichier, pour une barre de progression honnête :
    /// le décodeur de Qwen3-ASR pèse à lui seul les trois quarts du modèle.
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

/// Relaie l'avancement d'un téléchargement : l'API asynchrone d'URLSession
/// rend le fichier d'un coup, sans rien dire du chemin parcouru.
private final class DownloadProgress: NSObject, URLSessionDownloadDelegate {
    private let onProgress: (Int64) -> Void

    init(onProgress: @escaping (Int64) -> Void) {
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

    /// Exigée par le protocole ; l'API asynchrone récupère le fichier
    /// elle-même, il n'y a rien à faire ici.
    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didFinishDownloadingTo location: URL
    ) {}
}
