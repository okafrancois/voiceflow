import AVFoundation

/// Coupe du silence, équivalent du VAD de l'app actuelle : on ne transmet au
/// moteur que ce qui porte de la voix. Les silences de début et de fin
/// disparaissent, les pauses longues sont raccourcies.
///
/// Le principe : un seuil d'énergie, plus une traîne (« hangover ») qui laisse
/// passer un court instant après la fin de la parole, pour ne pas couper les
/// fins de mots.
struct SilenceTrimmer {
    /// Niveau (0…1) au-dessus duquel on considère qu'il y a de la voix.
    var threshold: Float = 0.06

    /// Délai au-delà duquel, faute d'avoir rien entendu, on laisse tout
    /// passer : mieux vaut transcrire du silence que perdre une dictée
    /// entière parce que le seuil est mal calibré pour ce micro.
    var failOpenAfter: TimeInterval = 1.5
    /// Durée de parole conservée après le passage sous le seuil.
    var hangover: TimeInterval = 0.45
    /// Durée de silence conservée à l'intérieur d'une pause longue.
    var keptPause: TimeInterval = 0.35

    init(threshold: Float = 0.06,
         hangover: TimeInterval = 0.45,
         keptPause: TimeInterval = 0.35,
         failOpenAfter: TimeInterval = 1.5) {
        self.threshold = threshold
        self.hangover = hangover
        self.keptPause = keptPause
        self.failOpenAfter = failOpenAfter
    }

    private(set) var hasHeardSpeech = false
    private var silenceStart: Date?
    private var lastSpeech = Date.distantPast
    private var started: Date?

    /// Faut-il transmettre ce bloc au moteur de transcription ?
    mutating func shouldPass(level: Float, now: Date = Date()) -> Bool {
        if started == nil { started = now }

        if level >= threshold {
            hasHeardSpeech = true
            lastSpeech = now
            silenceStart = nil
            return true
        }

        // Rien entendu depuis le début : on jette le silence initial, mais
        // pas indéfiniment — sinon un seuil trop haut ferait disparaître
        // toute la dictée.
        guard hasHeardSpeech else {
            guard let started, now.timeIntervalSince(started) > failOpenAfter else {
                return false
            }
            hasHeardSpeech = true
            lastSpeech = now
            return true
        }

        // Traîne après la parole : on laisse passer.
        if now.timeIntervalSince(lastSpeech) < hangover { return true }

        // Pause longue : on en garde un court fragment pour la respiration,
        // puis on coupe jusqu'à la reprise.
        if silenceStart == nil { silenceStart = now }
        guard let start = silenceStart else { return false }
        return now.timeIntervalSince(start) < keptPause
    }

    mutating func reset() {
        started = nil
        hasHeardSpeech = false
        silenceStart = nil
        lastSpeech = .distantPast
    }
}
