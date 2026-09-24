import AVFoundation

/// Convertit les blocs du micro vers le format qu'attend un moteur de
/// transcription. Chaque moteur a le sien : 16 kHz mono pour Whisper et
/// sherpa-onnx, celui que réclame SpeechAnalyzer pour le moteur d'Apple.
///
/// Le convertisseur est recréé si le format d'entrée change — ce qui arrive
/// quand l'utilisateur bascule de micro en cours de route.
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

    /// Format 16 kHz mono Float32, commun à Whisper et à sherpa-onnx.
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
