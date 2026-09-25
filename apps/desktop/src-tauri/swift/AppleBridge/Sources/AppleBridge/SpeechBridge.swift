import AVFoundation
import Foundation
import Speech

// C ABI over SpeechAnalyzer (macOS 26+). Every function except `feed` blocks
// the calling thread until the asynchronous Swift work completes, so Rust must
// call them from blocking threads.

private enum Status: Int32 {
    case ready = 0
    case assetsMissing = 1
    case localeUnsupported = 2
    case osUnsupported = 3
}

private func requestedLocale(_ identifier: UnsafePointer<CChar>) -> Locale {
    let value = String(cString: identifier)
    if value.isEmpty || value == "auto" {
        return Locale.current
    }
    return Locale(identifier: value)
}

@available(macOS 26, *)
private func resolveLocale(_ requested: Locale) async -> Locale? {
    if let equivalent = await SpeechTranscriber.supportedLocale(equivalentTo: requested) {
        return equivalent
    }
    let supported = await SpeechTranscriber.supportedLocales
    return supported.first { $0.language.languageCode == requested.language.languageCode }
}

@available(macOS 26, *)
private func makeTranscriber(_ locale: Locale, volatile: Bool) -> SpeechTranscriber {
    SpeechTranscriber(
        locale: locale,
        transcriptionOptions: [],
        reportingOptions: volatile ? [.volatileResults] : [],
        attributeOptions: []
    )
}

private enum SessionError: LocalizedError {
    case localeUnsupported
    case noAudioFormat
    case conversionUnavailable

    var errorDescription: String? {
        switch self {
        case .localeUnsupported: "The dictation language is not supported by Apple speech recognition"
        case .noAudioFormat: "No audio format is compatible with Apple speech recognition"
        case .conversionUnavailable: "Audio cannot be converted to the Apple speech recognition format"
        }
    }
}

/// One dictation: a SpeechAnalyzer fed with 16 kHz mono PCM.
@available(macOS 26, *)
private final class Session: @unchecked Sendable {
    private let analyzer: SpeechAnalyzer
    private let continuation: AsyncStream<AnalyzerInput>.Continuation
    private let inputFormat: AVAudioFormat
    private let analyzerFormat: AVAudioFormat
    private let converter: AVAudioConverter?
    private let resultsTask: Task<String, Error>
    private let lock = NSLock()

    static func start(locale requested: Locale) async throws -> Session {
        guard let locale = await resolveLocale(requested) else {
            throw SessionError.localeUnsupported
        }
        let transcriber = makeTranscriber(locale, volatile: false)
        if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
            try await request.downloadAndInstall()
        }
        guard let analyzerFormat = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber]) else {
            throw SessionError.noAudioFormat
        }
        return try await Session(transcriber: transcriber, analyzerFormat: analyzerFormat)
    }

    private init(transcriber: SpeechTranscriber, analyzerFormat: AVAudioFormat) async throws {
        guard let inputFormat = AVAudioFormat(
            commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false
        ) else {
            throw SessionError.conversionUnavailable
        }
        self.inputFormat = inputFormat
        self.analyzerFormat = analyzerFormat
        if inputFormat == analyzerFormat {
            converter = nil
        } else {
            guard let converter = AVAudioConverter(from: inputFormat, to: analyzerFormat) else {
                throw SessionError.conversionUnavailable
            }
            converter.primeMethod = .none
            self.converter = converter
        }

        analyzer = SpeechAnalyzer(modules: [transcriber])
        let (stream, continuation) = AsyncStream.makeStream(of: AnalyzerInput.self)
        self.continuation = continuation
        resultsTask = Task {
            var text = ""
            for try await result in transcriber.results where result.isFinal {
                text += String(result.text.characters)
            }
            return text
        }
        try await analyzer.start(inputSequence: stream)
    }

    func feed(_ samples: UnsafePointer<Int16>, count: Int) {
        guard count > 0 else { return }
        lock.lock()
        defer { lock.unlock() }

        guard let buffer = AVAudioPCMBuffer(pcmFormat: inputFormat, frameCapacity: AVAudioFrameCount(count)),
              let channel = buffer.floatChannelData?[0]
        else { return }
        for index in 0..<count {
            channel[index] = Float(samples[index]) / 32768
        }
        buffer.frameLength = AVAudioFrameCount(count)

        guard let converted = convert(buffer), converted.frameLength > 0 else { return }
        continuation.yield(AnalyzerInput(buffer: converted))
    }

    private func convert(_ buffer: AVAudioPCMBuffer) -> AVAudioPCMBuffer? {
        guard let converter else { return buffer }
        let ratio = analyzerFormat.sampleRate / inputFormat.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 16
        guard let output = AVAudioPCMBuffer(pcmFormat: analyzerFormat, frameCapacity: capacity) else {
            return nil
        }
        var consumed = false
        var error: NSError?
        converter.convert(to: output, error: &error) { _, status in
            if consumed {
                status.pointee = .noDataNow
                return nil
            }
            consumed = true
            status.pointee = .haveData
            return buffer
        }
        return error == nil ? output : nil
    }

    func finish() async throws -> String {
        continuation.finish()
        try await analyzer.finalizeAndFinishThroughEndOfInput()
        return try await resultsTask.value
    }

    func cancel() async {
        continuation.finish()
        await analyzer.cancelAndFinishNow()
        resultsTask.cancel()
    }
}

private final class SessionRegistry: @unchecked Sendable {
    static let shared = SessionRegistry()

    private let lock = NSLock()
    private var sessions: [Int64: AnyObject] = [:]
    private var nextID: Int64 = 1

    func insert(_ session: AnyObject) -> Int64 {
        lock.lock()
        defer { lock.unlock() }
        let id = nextID
        nextID += 1
        sessions[id] = session
        return id
    }

    func get(_ id: Int64) -> AnyObject? {
        lock.lock()
        defer { lock.unlock() }
        return sessions[id]
    }

    func remove(_ id: Int64) -> AnyObject? {
        lock.lock()
        defer { lock.unlock() }
        return sessions.removeValue(forKey: id)
    }
}

@_cdecl("vf_apple_speech_status")
public func vf_apple_speech_status(_ locale: UnsafePointer<CChar>) -> Int32 {
    guard #available(macOS 26, *), SpeechTranscriber.isAvailable else {
        return Status.osUnsupported.rawValue
    }
    let requested = requestedLocale(locale)
    return blocking {
        guard let resolved = await resolveLocale(requested) else {
            return Status.localeUnsupported.rawValue
        }
        let status = await AssetInventory.status(forModules: [makeTranscriber(resolved, volatile: false)])
        switch status {
        case .installed: return Status.ready.rawValue
        case .unsupported: return Status.localeUnsupported.rawValue
        default: return Status.assetsMissing.rawValue
        }
    }
}

@_cdecl("vf_apple_speech_install")
public func vf_apple_speech_install(_ locale: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar> {
    guard #available(macOS 26, *), SpeechTranscriber.isAvailable else {
        return duplicate("Apple speech recognition requires macOS 26 or later")
    }
    let requested = requestedLocale(locale)
    let message: String = blocking {
        guard let resolved = await resolveLocale(requested) else {
            return SessionError.localeUnsupported.localizedDescription
        }
        do {
            let transcriber = makeTranscriber(resolved, volatile: false)
            if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
                try await request.downloadAndInstall()
            }
            return ""
        } catch {
            return error.localizedDescription
        }
    }
    return duplicate(message)
}

@_cdecl("vf_apple_speech_start")
public func vf_apple_speech_start(
    _ locale: UnsafePointer<CChar>,
    _ error: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>
) -> Int64 {
    guard #available(macOS 26, *), SpeechTranscriber.isAvailable else {
        error.pointee = duplicate("Apple speech recognition requires macOS 26 or later")
        return 0
    }
    let requested = requestedLocale(locale)
    let result: Result<Session, Error> = blocking {
        do {
            return .success(try await Session.start(locale: requested))
        } catch {
            return .failure(error)
        }
    }
    switch result {
    case .success(let session):
        return SessionRegistry.shared.insert(session)
    case .failure(let failure):
        error.pointee = duplicate(failure.localizedDescription)
        return 0
    }
}

@_cdecl("vf_apple_speech_feed")
public func vf_apple_speech_feed(_ id: Int64, _ samples: UnsafePointer<Int16>, _ count: Int) {
    guard #available(macOS 26, *),
          let session = SessionRegistry.shared.get(id) as? Session
    else { return }
    session.feed(samples, count: count)
}

@_cdecl("vf_apple_speech_finish")
public func vf_apple_speech_finish(_ id: Int64) -> UnsafeMutablePointer<CChar> {
    guard #available(macOS 26, *),
          let session = SessionRegistry.shared.remove(id) as? Session
    else {
        return duplicate(json(["error": "Unknown Apple speech session"]))
    }
    let payload: [String: String] = blocking {
        do {
            return ["text": try await session.finish()]
        } catch {
            return ["error": error.localizedDescription]
        }
    }
    return duplicate(json(payload))
}

@_cdecl("vf_apple_speech_cancel")
public func vf_apple_speech_cancel(_ id: Int64) {
    guard #available(macOS 26, *),
          let session = SessionRegistry.shared.remove(id) as? Session
    else { return }
    blocking { await session.cancel() }
}
