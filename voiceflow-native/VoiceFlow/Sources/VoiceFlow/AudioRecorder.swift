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
    private var onStopped: (() -> Void)?
    private(set) var isRunning = false

    /// Ce que le fil audio écrit et que le fil principal lit.
    private struct Meter {
        var trimmer = SilenceTrimmer()
        var peak: Float = 0
        var passed = 0
        var dropped = 0
    }
    private let lock = NSLock()
    private var meter = Meter()

    /// Niveau le plus fort entendu pendant la prise. Reste à zéro quand macOS
    /// renvoie du silence — ce qu'il fait, sans erreur, si l'autorisation
    /// micro manque ou si l'entrée est coupée.
    var peakLevel: Float { lock.withLock { meter.peak } }

    struct Options {
        var deviceID: AudioDeviceID = AudioDevices.systemDefaultID
        var noiseReduction = true
        var trimSilence = true
        /// Sensibilité de la coupe du silence : écart exigé au-dessus du
        /// plancher de bruit. Plus haut, plus sévère.
        var silenceMargin: Float = 0.05
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
            if options.noiseReduction {
                // Le traitement vocal baisse par défaut le son des autres
                // apps comme pour un appel : pendant une dictée, la musique
                // n'a pas à s'effondrer.
                input.voiceProcessingOtherAudioDuckingConfiguration =
                    AVAudioVoiceProcessingOtherAudioDuckingConfiguration(
                        enableAdvancedDucking: false, duckingLevel: .min)
            }
        } catch {
            log.warning("voice processing unavailable: \(error.localizedDescription)")
        }

        lock.withLock { meter = Meter(trimmer: SilenceTrimmer(margin: options.silenceMargin)) }
        let trimSilence = options.trimSilence

        // Le format doit être relu après le changement de périphérique.
        let format = input.outputFormat(forBus: 0)
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, _ in
            let level = Self.level(of: buffer)
            onLevel?(level)
            guard let self else { return }
            let pass: Bool = self.lock.withLock {
                self.meter.peak = max(self.meter.peak, level)
                if trimSilence, !self.meter.trimmer.shouldPass(level: level) {
                    self.meter.dropped += 1
                    return false
                }
                self.meter.passed += 1
                return true
            }
            if pass { onBuffer(buffer) }
        }
        onStopped = { [weak self] in
            guard let self else { return }
            let meter = self.lock.withLock { self.meter }
            let total = max(1, meter.passed + meter.dropped)
            Diagnostics.log(
                "audio · \(meter.passed) blocs transmis, \(meter.dropped) coupés "
                + "(\(meter.dropped * 100 / total) %) · "
                + "crête \(String(format: "%.3f", meter.peak)) · "
                + "plancher \(String(format: "%.3f", meter.trimmer.noiseFloor)) · "
                + "seuil \(String(format: "%.3f", meter.trimmer.gate)) · "
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
