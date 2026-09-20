import AVFoundation

/// Un moteur de dictée reçoit les buffers micro pendant l'enregistrement
/// et rend le texte final à la fin.
protocol DictationEngine {
    func feed(_ buffer: AVAudioPCMBuffer)
    func finish() async throws -> String
}

extension TranscriptionSession: DictationEngine {}

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

    var id: String { rawValue }

    /// Du plus léger au plus lourd : c'est l'ordre d'affichage, et c'est
    /// aussi, à peu près, l'ordre de vitesse.
    static let whisperChoices: [EngineChoice] = [
        .whisperTiny, .whisperTinyEN,
        .whisperBase, .whisperBaseEN,
        .whisperSmall, .whisperSmallEN,
        .whisperDistilLarge, .whisperLargeTurbo,
        .whisperMedium, .whisperMediumEN,
        .whisperLargeV2, .whisperLarge,
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
        }
    }

    var isWhisper: Bool { whisperModel != nil }

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
        guard isWhisper else { return appleLocales }
        guard !isEnglishOnly else { return ["en"] }
        return Constants.languages.values
            .map { String($0) }
            .reduce(into: Set<String>()) { $0.insert($1) }
            .sorted()
    }
}
