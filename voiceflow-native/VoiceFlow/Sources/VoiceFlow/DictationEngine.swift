import AVFoundation

/// Un moteur de dictée reçoit les buffers micro pendant l'enregistrement
/// et rend le texte final à la fin.
///
/// Partagé entre fils : l'audio arrive du fil du micro (toujours sous le
/// verrou d'`EngineFeed`), la fin est demandée depuis l'app.
protocol DictationEngine: AnyObject, Sendable {
    func feed(_ buffer: AVAudioPCMBuffer)
    func finish() async throws -> String
    /// Dictée abandonnée : libérer ce qui tourne encore.
    func cancel() async
}

extension DictationEngine {
    func cancel() async {}
}

extension TranscriptionSession: DictationEngine {}

/// Exécute des travaux asynchrones un par un, dans l'ordre d'arrivée.
///
/// Une dictée annulée pendant sa transcription laisse la main à la
/// suivante : deux décodages pourraient alors se croiser sur le même
/// modèle, que ni WhisperKit ni sherpa-onnx ne garantissent pour cet usage.
actor SerialWork {
    private var tail: Task<Void, Never>?

    func run<T: Sendable>(_ work: @escaping @Sendable () async throws -> T) async throws -> T {
        let previous = tail
        let task = Task<T, Error> {
            await previous?.value
            return try await work()
        }
        tail = Task { _ = try? await task.value }
        return try await task.value
    }
}

/// Relais entre le micro et le moteur.
///
/// Le micro démarre dès la pression du raccourci, le moteur peut mettre
/// plusieurs centaines de millisecondes à être prêt (celui d'Apple interroge
/// ses modèles installés) : tout ce qui est dit dans cet intervalle est gardé
/// ici, puis transmis dans l'ordre dès que le moteur arrive.
///
/// Les blocs mis en attente sont copiés : le tap d'`AVAudioEngine` peut
/// réutiliser sa mémoire une fois le rappel terminé.
final class EngineFeed: @unchecked Sendable {
    private let lock = NSLock()
    private var pending: [AVAudioPCMBuffer] = []
    private var engine: DictationEngine?

    /// Échantillons en attente du moteur, pour le journal.
    var pendingFrames: Int {
        lock.withLock { pending.reduce(0) { $0 + Int($1.frameLength) } }
    }

    /// Appelé depuis le fil audio. Le verrou couvre aussi la transmission au
    /// moteur : sans lui, un bloc frais pourrait doubler la file vidée par
    /// `attach`.
    func push(_ buffer: AVAudioPCMBuffer) {
        lock.withLock {
            if let engine {
                engine.feed(buffer)
            } else if let copy = Self.copy(buffer) {
                pending.append(copy)
            }
        }
    }

    func attach(_ engine: DictationEngine) {
        lock.withLock {
            for buffer in pending { engine.feed(buffer) }
            pending = []
            self.engine = engine
        }
    }

    private static func copy(_ buffer: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
        guard let copy = AVAudioPCMBuffer(
            pcmFormat: buffer.format, frameCapacity: buffer.frameLength)
        else { return nil }
        copy.frameLength = buffer.frameLength
        let source = UnsafeMutableAudioBufferListPointer(
            UnsafeMutablePointer(mutating: buffer.audioBufferList))
        let target = UnsafeMutableAudioBufferListPointer(copy.mutableAudioBufferList)
        for (from, to) in zip(source, target) {
            guard let fromData = from.mData, let toData = to.mData else { continue }
            memcpy(toData, fromData, Int(min(from.mDataByteSize, to.mDataByteSize)))
        }
        return copy
    }
}

/// Choix de moteur exposé dans le menu : le moteur système d'Apple, ou un
/// modèle Whisper (WhisperKit). Conçu pour accueillir d'autres familles de
/// moteurs plus tard (ajouter un cas + un DictationEngine).
///
/// Les `rawValue` sont enregistrés dans les réglages : ne pas les renommer.
enum EngineChoice: String, CaseIterable, Identifiable {
    case apple = "apple"
    case whisperTiny = "whisper-tiny"
    case whisperTinyEN = "whisper-tiny-en"
    case whisperBase = "whisper-base"
    case whisperBaseEN = "whisper-base-en"
    case whisperSmall = "whisper-small"
    case whisperSmallEN = "whisper-small-en"
    case whisperDistilLarge = "whisper-distil-large-v3"
    case whisperLargeTurbo = "whisper-large-v3-turbo"
    case whisperMedium = "whisper-medium"
    case whisperMediumEN = "whisper-medium-en"
    case whisperLargeV2 = "whisper-large-v2"
    case whisperLarge = "whisper-large-v3"
    case senseVoice = "sense-voice-small"
    case qwen3ASR = "qwen3-asr-0.6b"

    var id: String { rawValue }

    /// Tout ce qui se télécharge : la famille Whisper, du plus léger au
    /// plus lourd, puis les moteurs ONNX portés de l'app Tauri.
    static let downloadableChoices: [EngineChoice] = [
        .whisperTiny, .whisperTinyEN,
        .whisperBase, .whisperBaseEN,
        .whisperSmall, .whisperSmallEN,
        .whisperDistilLarge, .whisperLargeTurbo,
        .whisperMedium, .whisperMediumEN,
        .whisperLargeV2, .whisperLarge,
        .senseVoice, .qwen3ASR,
    ]

    /// Nom complet : chaque entrée doit se suffire à elle-même dans un menu.
    var displayName: String {
        switch self {
        case .apple: L.t("SpeechAnalyzer (Apple)")
        case .whisperTiny: L.t("Whisper Tiny")
        case .whisperTinyEN: L.t("Whisper Tiny (anglais)")
        case .whisperBase: L.t("Whisper Base")
        case .whisperBaseEN: L.t("Whisper Base (anglais)")
        case .whisperSmall: L.t("Whisper Small")
        case .whisperSmallEN: L.t("Whisper Small (anglais)")
        case .whisperDistilLarge: L.t("Distil-Whisper Large v3 (anglais)")
        case .whisperLargeTurbo: L.t("Whisper Large v3 Turbo")
        case .whisperMedium: L.t("Whisper Medium")
        case .whisperMediumEN: L.t("Whisper Medium (anglais)")
        case .whisperLargeV2: L.t("Whisper Large v2")
        case .whisperLarge: L.t("Whisper Large v3")
        case .senseVoice: L.t("SenseVoice Small")
        case .qwen3ASR: L.t("Qwen3-ASR 0.6B")
        }
    }

    var detail: String {
        switch self {
        case .apple: L.t("Modèle du système · aucun téléchargement · transcription en direct")
        case .whisperTiny: L.t("≈ 75 Mo · le plus rapide")
        case .whisperTinyEN: L.t("≈ 75 Mo · le plus rapide, anglais uniquement")
        case .whisperBase: L.t("≈ 145 Mo")
        case .whisperBaseEN: L.t("≈ 145 Mo · anglais uniquement")
        case .whisperSmall: L.t("≈ 480 Mo · équilibré")
        case .whisperSmallEN: L.t("≈ 480 Mo · équilibré, anglais uniquement")
        case .whisperDistilLarge: L.t("≈ 600 Mo · très rapide, anglais uniquement")
        case .whisperLargeTurbo: L.t("≈ 645 Mo · presque la précision de Large, bien plus rapide")
        case .whisperMedium: L.t("≈ 1,5 Go")
        case .whisperMediumEN: L.t("≈ 1,5 Go · anglais uniquement")
        case .whisperLargeV2: L.t("≈ 3 Go · génération précédente")
        case .whisperLarge: L.t("≈ 3 Go · précision maximale")
        case .senseVoice:
            L.t("≈ 240 Mo · rapide · chinois, cantonais, japonais, coréen, anglais")
        case .qwen3ASR: L.t("≈ 985 Mo · multilingue · ponctuation soignée")
        }
    }

    /// Variante exacte du dépôt `argmaxinc/whisperkit-coreml`.
    var whisperModel: String? {
        switch self {
        case .apple: nil
        case .whisperTiny: "openai_whisper-tiny"
        case .whisperTinyEN: "openai_whisper-tiny.en"
        case .whisperBase: "openai_whisper-base"
        case .whisperBaseEN: "openai_whisper-base.en"
        case .whisperSmall: "openai_whisper-small"
        case .whisperSmallEN: "openai_whisper-small.en"
        case .whisperDistilLarge: "distil-whisper_distil-large-v3_turbo_600MB"
        case .whisperLargeTurbo: "openai_whisper-large-v3-v20240930_turbo_632MB"
        case .whisperMedium: "openai_whisper-medium"
        case .whisperMediumEN: "openai_whisper-medium.en"
        case .whisperLargeV2: "openai_whisper-large-v2"
        case .whisperLarge: "openai_whisper-large-v3"
        case .senseVoice, .qwen3ASR: nil
        }
    }

    /// Modèle ONNX exécuté par sherpa-onnx, le cas échéant.
    var sherpaModel: SherpaModel? {
        switch self {
        case .senseVoice: .senseVoice
        case .qwen3ASR: .qwen3ASR
        default: nil
        }
    }

    var isWhisper: Bool { whisperModel != nil }

    /// Le moteur sait-il reconnaître seul la langue parlée ? Celui d'Apple
    /// ne le fait pas : il transcrit dans la langue qu'on lui donne.
    var detectsLanguage: Bool { self != .apple }

    /// Étiquette courte affichée pendant la transcription.
    var shortLabel: String {
        switch self {
        case .apple: L.t("Transcription Apple")
        case .senseVoice: L.t("Transcription SenseVoice")
        case .qwen3ASR: L.t("Transcription Qwen3-ASR")
        default: L.t("Transcription Whisper")
        }
    }

    /// Noms enregistrés dans l'historique des dictées.
    var historyEngine: String {
        switch self {
        case .apple: "apple"
        case .senseVoice: "sensevoice"
        case .qwen3ASR: "qwen3-asr"
        default: "whisper"
        }
    }

    var historyModel: String? { whisperModel ?? sherpaModel?.id }

    /// Nom affichable d'un moteur tel qu'enregistré dans l'historique.
    static func historyDisplayName(_ engine: String) -> String {
        switch engine {
        case "apple": "Apple"
        case "whisper": "Whisper"
        case "sensevoice": "SenseVoice"
        case "qwen3-asr": "Qwen3-ASR"
        default: engine
        }
    }

    /// Modèles entraînés sur l'anglais seul : proposer d'autres langues
    /// donnerait une transcription silencieusement fausse.
    var isEnglishOnly: Bool {
        whisperModel.map { $0.hasSuffix(".en") || $0.hasPrefix("distil-whisper") } ?? false
    }

    /// Le moteur système fournit des résultats provisoires en continu ;
    /// Whisper transcrit en une passe à la fin de l'enregistrement.
    var supportsVolatileResults: Bool { self == .apple }
}

import WhisperKit

extension EngineChoice {
    /// Langues proposées pour ce moteur, en identifiants BCP-47.
    ///
    /// Apple ne transcrit que dans les locales de ses modèles installés ;
    /// Whisper en couvre une centaine, indépendamment de macOS — sauf les
    /// variantes anglaises, qui ne couvrent qu'elle.
    func supportedLocaleIDs(appleLocales: [String]) -> [String] {
        if let sherpa = sherpaModel { return sherpa.supportedLanguages.sorted() }
        guard isWhisper else { return appleLocales }
        guard !isEnglishOnly else { return ["en"] }
        return Constants.languages.values
            .map { String($0) }
            .reduce(into: Set<String>()) { $0.insert($1) }
            .sorted()
    }
}
