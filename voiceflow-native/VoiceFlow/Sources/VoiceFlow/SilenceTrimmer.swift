import AVFoundation

/// Coupe du silence : on ne transmet au moteur que ce qui porte de la voix.
/// Les silences de début et de fin disparaissent, les pauses longues sont
/// raccourcies.
///
/// Le seuil n'est pas absolu — il ne peut pas l'être. Le niveau de sortie
/// dépend du micro, de la distance et du gain d'entrée : un seuil qui convient
/// à un casque proche taille en pleine phrase sur un micro intégré. On suit
/// donc le plancher de bruit de la prise et on ouvre la porte à un écart fixe
/// au-dessus de lui.
///
/// Le réglage est volontairement asymétrique : laisser passer du silence ne
/// coûte qu'un peu de calcul, couper de la parole coûte des mots. Une pause
/// plus courte que `hangover + keptPause` passe donc toujours en entier.
struct SilenceTrimmer {
    /// Écart au-dessus du plancher de bruit à partir duquel on considère qu'il
    /// y a de la voix. Exprimé dans l'échelle de niveau normalisée, où 1
    /// couvre 50 dB : 0,05 vaut donc environ 2,5 dB.
    var margin: Float = 0.05

    /// Délai au-delà duquel, faute d'avoir rien entendu, on laisse tout
    /// passer : mieux vaut transcrire du silence que perdre une dictée
    /// entière parce que le micro est plus sourd que prévu.
    var failOpenAfter: TimeInterval = 1.5
    /// Durée conservée après le passage sous le seuil.
    var hangover: TimeInterval = 1.2
    /// Durée de silence conservée à l'intérieur d'une pause longue.
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
    /// Plancher de bruit estimé, pour le journal de diagnostic.
    private(set) var noiseFloor: Float = 0
    private var calibrated = false
    private var silenceStart: Date?
    private var lastSpeech = Date.distantPast
    private var started: Date?

    /// Seuil effectif du moment : jamais sous un plancher absolu, pour ne pas
    /// transmettre du silence numérique quand l'entrée est muette.
    var gate: Float { max(0.02, noiseFloor + margin) }

    /// Faut-il transmettre ce bloc au moteur de transcription ?
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

        // Rien entendu depuis le début : on jette le silence initial, mais
        // pas indéfiniment — sinon un seuil mal calibré ferait disparaître
        // toute la dictée.
        guard hasHeardSpeech else {
            guard let started, now.timeIntervalSince(started) > failOpenAfter else {
                return false
            }
            hasHeardSpeech = true
            lastSpeech = now
            return true
        }

        // Traîne après la parole : on laisse passer. C'est elle qui garantit
        // qu'une respiration, une hésitation ou une consonne sourde ne coupe
        // jamais la phrase en deux.
        if now.timeIntervalSince(lastSpeech) < hangover { return true }

        // Pause longue : on en garde un court fragment pour la respiration,
        // puis on coupe jusqu'à la reprise.
        if silenceStart == nil { silenceStart = now }
        guard let start = silenceStart else { return false }
        return now.timeIntervalSince(start) < keptPause
    }

    /// Le plancher descend vite vers le niveau courant et remonte très
    /// lentement : il se cale sur les creux de la prise, jamais sur la voix.
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
            // Pendant la parole le plancher ne bouge quasiment pas, sinon la
            // porte se refermerait sur celui qui parle fort.
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
