import AVFoundation

/// Silence trimming: only what carries voice is forwarded to the engine.
/// Leading and trailing silence disappears, long pauses are shortened.
///
/// The threshold isn't absolute — it can't be. Output level depends on the
/// microphone, the distance, and the input gain: a threshold that fits a
/// close headset clips mid-sentence on a built-in mic. So we track the
/// take's noise floor and open the gate at a fixed margin above it.
///
/// The tuning is deliberately asymmetric: letting silence through only
/// costs a bit of compute, cutting speech costs words. A pause shorter
/// than `hangover + keptPause` therefore always passes through whole.
struct SilenceTrimmer {
    /// Margin above the noise floor beyond which we consider it voice.
    /// Expressed on the normalized level scale, where 1 covers 50 dB:
    /// 0.05 is therefore about 2.5 dB.
    var margin: Float = 0.05

    /// Delay beyond which, having heard nothing, we let everything
    /// through: better to transcribe silence than lose an entire
    /// dictation because the mic is quieter than expected.
    var failOpenAfter: TimeInterval = 1.5
    /// Duration kept after dropping below the threshold.
    var hangover: TimeInterval = 1.2
    /// Duration of silence kept inside a long pause.
    var keptPause: TimeInterval = 0.6

    init(margin: Float = 0.05,
         hangover: TimeInterval = 1.2,
         keptPause: TimeInterval = 0.6,
         failOpenAfter: TimeInterval = 1.5) {
        self.margin = margin
        self.hangover = hangover
        self.keptPause = keptPause
        self.failOpenAfter = failOpenAfter
    }

    private(set) var hasHeardSpeech = false
    /// Estimated noise floor, for the diagnostic log.
    private(set) var noiseFloor: Float = 0
    private var calibrated = false
    private var silenceStart: Date?
    private var lastSpeech = Date.distantPast
    private var started: Date?

    /// Current effective threshold: never below an absolute floor, so we
    /// don't forward digital silence when the input is muted.
    var gate: Float { max(0.02, noiseFloor + margin) }

    /// Should this block be forwarded to the transcription engine?
    mutating func shouldPass(level: Float, now: Date = Date()) -> Bool {
        if started == nil { started = now }

        let isSpeech = level >= gate
        updateNoiseFloor(with: level, isSpeech: isSpeech)

        if isSpeech {
            hasHeardSpeech = true
            lastSpeech = now
            silenceStart = nil
            return true
        }

        // Nothing heard since the start: we drop the initial silence, but
        // not indefinitely — otherwise a poorly calibrated threshold would
        // make the whole dictation disappear.
        guard hasHeardSpeech else {
            guard let started, now.timeIntervalSince(started) > failOpenAfter else {
                return false
            }
            hasHeardSpeech = true
            lastSpeech = now
            return true
        }

        // Tail after speech: we let it through. This is what guarantees
        // that a breath, a hesitation, or a voiceless consonant never
        // splits the sentence in two.
        if now.timeIntervalSince(lastSpeech) < hangover { return true }

        // Long pause: we keep a short fragment of it for the breath,
        // then cut until speech resumes.
        if silenceStart == nil { silenceStart = now }
        guard let start = silenceStart else { return false }
        return now.timeIntervalSince(start) < keptPause
    }

    /// The floor drops quickly toward the current level and rises very
    /// slowly: it settles on the take's quiet spots, never on the voice.
    private mutating func updateNoiseFloor(with level: Float, isSpeech: Bool) {
        guard calibrated else {
            noiseFloor = level
            calibrated = true
            return
        }
        if level < noiseFloor {
            noiseFloor += (level - noiseFloor) * 0.3
        } else if !isSpeech {
            noiseFloor += (level - noiseFloor) * 0.05
        } else {
            // During speech the floor barely moves, otherwise the gate
            // would close on someone speaking loudly.
            noiseFloor += (level - noiseFloor) * 0.0015
        }
    }

    mutating func reset() {
        started = nil
        hasHeardSpeech = false
        calibrated = false
        noiseFloor = 0
        silenceStart = nil
        lastSpeech = .distantPast
    }
}
