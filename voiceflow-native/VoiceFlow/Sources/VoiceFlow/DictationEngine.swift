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
enum EngineChoice: String, CaseIterable, Identifiable {
    case apple = "apple"
    case whisperTiny = "whisper-tiny"
    case whisperBase = "whisper-base"
    case whisperSmall = "whisper-small"
    case whisperLarge = "whisper-large-v3"

    var id: String { rawValue }

    static let whisperChoices: [EngineChoice] = [
        .whisperTiny, .whisperBase, .whisperSmall, .whisperLarge,
    ]

    /// Nom complet : chaque entrée doit se suffire à elle-même dans un menu.
    var displayName: String {
        switch self {
        case .apple: L.t("SpeechAnalyzer (Apple)")
        case .whisperTiny: L.t("Whisper Tiny")
        case .whisperBase: L.t("Whisper Base")
        case .whisperSmall: L.t("Whisper Small")
        case .whisperLarge: L.t("Whisper Large v3")
        }
    }

    var detail: String {
        switch self {
        case .apple: L.t("Modèle du système · aucun téléchargement · transcription en direct")
        case .whisperTiny: L.t("≈ 75 Mo · le plus rapide")
        case .whisperBase: L.t("≈ 145 Mo")
        case .whisperSmall: L.t("≈ 480 Mo · équilibré")
        case .whisperLarge: L.t("≈ 3 Go · précision maximale")
        }
    }

    /// Variante exacte du dépôt `argmaxinc/whisperkit-coreml`.
    var whisperModel: String? {
        switch self {
        case .apple: nil
        case .whisperTiny: "openai_whisper-tiny"
        case .whisperBase: "openai_whisper-base"
        case .whisperSmall: "openai_whisper-small"
        case .whisperLarge: "openai_whisper-large-v3"
        }
    }

    var isWhisper: Bool { whisperModel != nil }

    /// Le moteur système fournit des résultats provisoires en continu ;
    /// Whisper transcrit en une passe à la fin de l'enregistrement.
    var supportsVolatileResults: Bool { self == .apple }
}

import WhisperKit

extension EngineChoice {
    /// Langues proposées pour ce moteur, en identifiants BCP-47.
    ///
    /// Apple ne transcrit que dans les locales de ses modèles installés ;
    /// Whisper en couvre une centaine, indépendamment de macOS.
    func supportedLocaleIDs(appleLocales: [String]) -> [String] {
        guard isWhisper else { return appleLocales }
        return Constants.languages.values
            .map { String($0) }
            .reduce(into: Set<String>()) { $0.insert($1) }
            .sorted()
    }
}
