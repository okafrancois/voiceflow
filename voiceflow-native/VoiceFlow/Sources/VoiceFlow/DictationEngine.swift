import AVFoundation

/// A dictation engine receives microphone buffers during recording and
/// returns the final text at the end.
///
/// Shared across threads: audio arrives from the microphone thread
/// (always under `EngineFeed`'s lock), completion is requested from the
/// app.
protocol DictationEngine: AnyObject, Sendable {
    func feed(_ buffer: AVAudioPCMBuffer)
    func finish() async throws -> String
    /// Dictation abandoned: release whatever is still running.
    func cancel() async
}

extension DictationEngine {
    func cancel() async {}
}

extension TranscriptionSession: DictationEngine {}

/// Runs asynchronous jobs one by one, in arrival order.
///
/// A dictation cancelled during its transcription hands off to the next
/// one: otherwise two decodings could overlap on the same model, which
/// neither WhisperKit nor sherpa-onnx guarantees for this use case.
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

/// Relay between the microphone and the engine.
///
/// The microphone starts as soon as the shortcut is pressed, the engine
/// can take several hundred milliseconds to be ready (Apple's queries its
/// installed models): anything said in that interval is kept here, then
/// forwarded in order as soon as the engine arrives.
///
/// Buffered blocks are copied: `AVAudioEngine`'s tap can reuse its memory
/// once the callback returns.
final class EngineFeed: @unchecked Sendable {
    private let lock = NSLock()
    private var pending: [AVAudioPCMBuffer] = []
    private var engine: DictationEngine?

    /// Samples still waiting on the engine, for the log.
    var pendingFrames: Int {
        lock.withLock { pending.reduce(0) { $0 + Int($1.frameLength) } }
    }

    /// Called from the audio thread. The lock also covers forwarding to
    /// the engine: without it, a fresh block could duplicate the queue
    /// drained by `attach`.
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

/// Engine choice exposed in the menu: Apple's system engine, or a
/// Whisper model (WhisperKit). Designed to accommodate other engine
/// families later (add a case + a DictationEngine).
///
/// `rawValue`s are stored in settings: do not rename them.
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

    /// Everything downloadable: the Whisper family, from lightest to
    /// heaviest, then the ONNX engines ported from the Tauri app.
    static let downloadableChoices: [EngineChoice] = [
        .whisperTiny, .whisperTinyEN,
        .whisperBase, .whisperBaseEN,
        .whisperSmall, .whisperSmallEN,
        .whisperDistilLarge, .whisperLargeTurbo,
        .whisperMedium, .whisperMediumEN,
        .whisperLargeV2, .whisperLarge,
        .senseVoice, .qwen3ASR,
    ]

    /// Full name: each entry must stand on its own in a menu.
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

    /// Exact variant from the `argmaxinc/whisperkit-coreml` repo.
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

    /// ONNX model run by sherpa-onnx, if any.
    var sherpaModel: SherpaModel? {
        switch self {
        case .senseVoice: .senseVoice
        case .qwen3ASR: .qwen3ASR
        default: nil
        }
    }

    var isWhisper: Bool { whisperModel != nil }

    /// Can the engine detect the spoken language on its own? Apple's
    /// cannot: it transcribes in whichever language it's given.
    var detectsLanguage: Bool { self != .apple }

    /// Short label shown while transcribing.
    var shortLabel: String {
        switch self {
        case .apple: L.t("Transcription Apple")
        case .senseVoice: L.t("Transcription SenseVoice")
        case .qwen3ASR: L.t("Transcription Qwen3-ASR")
        default: L.t("Transcription Whisper")
        }
    }

    /// Names stored in the dictation history.
    var historyEngine: String {
        switch self {
        case .apple: "apple"
        case .senseVoice: "sensevoice"
        case .qwen3ASR: "qwen3-asr"
        default: "whisper"
        }
    }

    var historyModel: String? { whisperModel ?? sherpaModel?.id }

    /// Display name of an engine as stored in the history.
    static func historyDisplayName(_ engine: String) -> String {
        switch engine {
        case "apple": "Apple"
        case "whisper": "Whisper"
        case "sensevoice": "SenseVoice"
        case "qwen3-asr": "Qwen3-ASR"
        default: engine
        }
    }

    /// Models trained on English only: offering other languages would
    /// give a silently wrong transcription.
    var isEnglishOnly: Bool {
        whisperModel.map { $0.hasSuffix(".en") || $0.hasPrefix("distil-whisper") } ?? false
    }

    /// The system engine streams volatile results continuously; Whisper
    /// transcribes in one pass at the end of the recording.
    var supportsVolatileResults: Bool { self == .apple }
}

import WhisperKit

extension EngineChoice {
    /// Languages offered for this engine, as BCP-47 identifiers.
    ///
    /// Apple only transcribes in the locales of its installed models;
    /// Whisper covers about a hundred, independently of macOS — except
    /// for the English variants, which only cover that one language.
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
