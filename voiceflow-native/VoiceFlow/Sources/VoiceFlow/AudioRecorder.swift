import AVFoundation
import CoreAudio

/// Microphone capture via AVAudioEngine.
///
/// Three settings act here: the input device, noise reduction (the
/// system's voice processing, which also brings echo cancellation), and
/// silence trimming, which avoids sending the engine anything that carries
/// no voice.
final class AudioRecorder {
    private let engine = AVAudioEngine()
    private var onStopped: (() -> Void)?
    private(set) var isRunning = false

    /// What the audio thread writes and the main thread reads.
    private struct Meter {
        var trimmer = SilenceTrimmer()
        var peak: Float = 0
        var passed = 0
        var dropped = 0
    }
    private let lock = NSLock()
    private var meter = Meter()

    /// Loudest level heard during the take. Stays at zero when macOS
    /// returns silence — which it does, without error, if microphone
    /// permission is missing or the input is muted.
    var peakLevel: Float { lock.withLock { meter.peak } }

    struct Options {
        var deviceID: AudioDeviceID = AudioDevices.systemDefaultID
        var noiseReduction = true
        var trimSilence = true
        /// Sensitivity of silence trimming: the margin required above
        /// the noise floor. Higher means stricter.
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

        // System voice processing: noise reduction and echo cancellation,
        // with no model to bundle.
        do {
            try input.setVoiceProcessingEnabled(options.noiseReduction)
            if options.noiseReduction {
                // Voice processing ducks other apps' audio by default, as
                // for a call: during a dictation, music shouldn't be
                // crushed.
                input.voiceProcessingOtherAudioDuckingConfiguration =
                    AVAudioVoiceProcessingOtherAudioDuckingConfiguration(
                        enableAdvancedDucking: false, duckingLevel: .min)
            }
        } catch {
            log.warning("voice processing unavailable: \(error.localizedDescription)")
        }

        lock.withLock { meter = Meter(trimmer: SilenceTrimmer(margin: options.silenceMargin)) }
        let trimSilence = options.trimSilence

        // The format must be re-read after the device change.
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
                "audio · \(meter.passed) blocks passed, \(meter.dropped) dropped "
                + "(\(meter.dropped * 100 / total) %) · "
                + "peak \(String(format: "%.3f", meter.peak)) · "
                + "floor \(String(format: "%.3f", meter.trimmer.noiseFloor)) · "
                + "gate \(String(format: "%.3f", meter.trimmer.gate)) · "
                + "noise reduction \(options.noiseReduction) · silence trim \(trimSilence)")
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

    /// Switches the input unit to a specific device.
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

    /// Normalized level 0…1 (RMS → dB, floor at −50 dB).
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
