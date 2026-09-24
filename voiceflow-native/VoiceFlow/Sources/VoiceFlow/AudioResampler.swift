import AVFoundation

/// Converts microphone blocks to the format a transcription engine
/// expects. Each engine has its own: 16 kHz mono for Whisper and
/// sherpa-onnx, whatever SpeechAnalyzer requires for the Apple engine.
///
/// The converter is recreated if the input format changes — which
/// happens when the user switches microphones mid-way.
final class AudioResampler {
    enum ResamplerError: LocalizedError {
        case unavailable

        var errorDescription: String? {
            switch self {
            case .unavailable: L.t("Conversion audio impossible vers le format du moteur")
            }
        }
    }

    private let target: AVAudioFormat
    private var converter: AVAudioConverter?

    init(to target: AVAudioFormat) {
        self.target = target
    }

    /// 16 kHz mono Float32 format, shared by Whisper and sherpa-onnx.
    static func standard16k() throws -> AVAudioFormat {
        guard let format = AVAudioFormat(
            commonFormat: .pcmFormatFloat32, sampleRate: 16000, channels: 1, interleaved: false
        ) else {
            throw ResamplerError.unavailable
        }
        return format
    }

    func convert(_ buffer: AVAudioPCMBuffer) throws -> AVAudioPCMBuffer {
        if buffer.format == target { return buffer }

        if converter == nil || converter?.inputFormat != buffer.format {
            converter = AVAudioConverter(from: buffer.format, to: target)
            converter?.primeMethod = .none
        }
        guard let converter else { throw ResamplerError.unavailable }

        let ratio = target.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 16
        guard let output = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: capacity) else {
            throw ResamplerError.unavailable
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
        if let conversionError { throw conversionError }
        return output
    }
}
