import AVFoundation
import CoreAudio

/// Capture micro via AVAudioEngine.
///
/// Trois réglages agissent ici : le périphérique d'entrée, la réduction de
/// bruit (traitement vocal du système, qui apporte aussi l'annulation d'écho)
/// et la coupe du silence, qui évite d'envoyer au moteur ce qui ne porte pas
/// de voix.
final class AudioRecorder {
    private let engine = AVAudioEngine()
    private var trimmer = SilenceTrimmer()
    private var onStopped: (() -> Void)?
    private(set) var isRunning = false

    /// Niveau le plus fort entendu pendant la prise. Reste à zéro quand macOS
    /// renvoie du silence — ce qu'il fait, sans erreur, si l'autorisation
    /// micro manque ou si l'entrée est coupée.
    private(set) var peakLevel: Float = 0

    struct Options {
        var deviceID: AudioDeviceID = AudioDevices.systemDefaultID
        var noiseReduction = true
        var trimSilence = true
        /// Sensibilité de la coupe du silence (0…1) : plus haut, plus sévère.
        var silenceThreshold: Float = 0.12
    }

    func start(
        options: Options,
        onBuffer: @escaping (AVAudioPCMBuffer) -> Void,
        onLevel: ((Float) -> Void)? = nil
    ) throws {
        let input = engine.inputNode

        if options.deviceID != AudioDevices.systemDefaultID,
           AudioDevices.exists(options.deviceID) {
            try setInputDevice(options.deviceID, on: input)
        }

        // Traitement vocal du système : réduction de bruit et annulation
        // d'écho, sans modèle à embarquer.
        do {
            try input.setVoiceProcessingEnabled(options.noiseReduction)
        } catch {
            log.warning("voice processing unavailable: \(error.localizedDescription)")
        }

        trimmer = SilenceTrimmer(threshold: options.silenceThreshold)
        let trimSilence = options.trimSilence

        // Le format doit être relu après le changement de périphérique.
        let format = input.outputFormat(forBus: 0)
        peakLevel = 0
        var passed = 0
        var dropped = 0
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, _ in
            let level = Self.level(of: buffer)
            onLevel?(level)
            guard let self else { return }
            self.peakLevel = max(self.peakLevel, level)
            if trimSilence, !self.trimmer.shouldPass(level: level) {
                dropped += 1
                return
            }
            passed += 1
            onBuffer(buffer)
        }
        onStopped = { [weak self] in
            Diagnostics.log(
                "audio · \(passed) blocs transmis, \(dropped) coupés · "
                + "crête \(String(format: "%.3f", self?.peakLevel ?? 0)) · "
                + "bruit \(options.noiseReduction) · silence \(trimSilence)")
        }

        engine.prepare()
        try engine.start()
        isRunning = true
    }

    func stop() {
        guard isRunning else { return }
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        isRunning = false
        onStopped?()
        onStopped = nil
    }

    /// Bascule l'unité d'entrée sur un périphérique précis.
    private func setInputDevice(_ deviceID: AudioDeviceID, on input: AVAudioInputNode) throws {
        guard let unit = input.audioUnit else { return }
        var id = deviceID
        let status = AudioUnitSetProperty(
            unit,
            kAudioOutputUnitProperty_CurrentDevice,
            kAudioUnitScope_Global,
            0,
            &id,
            UInt32(MemoryLayout<AudioDeviceID>.size))
        if status != noErr {
            log.warning("input device selection failed (status \(status)), using system default")
        }
    }

    /// Niveau normalisé 0…1 (RMS → dB, plancher à −50 dB).
    private static func level(of buffer: AVAudioPCMBuffer) -> Float {
        guard let data = buffer.floatChannelData?[0], buffer.frameLength > 0 else { return 0 }
        let frames = Int(buffer.frameLength)
        var sum: Float = 0
        for index in 0..<frames {
            sum += data[index] * data[index]
        }
        let rms = sqrt(sum / Float(frames))
        let db = 20 * log10(max(rms, .leastNonzeroMagnitude))
        return max(0, min(1, (db + 50) / 50))
    }
}
