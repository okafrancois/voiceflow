import AppKit
import Foundation

/// Apprentissage des corrections : après une insertion, on relit le champ
/// cible et on compare mot à mot avec ce qui avait été écrit. Les mots que
/// l'utilisateur a remplacés deviennent des entrées de dictionnaire.
///
/// Volontairement prudent : une seule relecture différée, uniquement sur des
/// substitutions mot pour mot de longueur comparable. Tout le reste (phrases
/// réécrites, texte effacé, ajouts) est ignoré, car on ne saurait pas en
/// déduire une correction fiable.
@MainActor
enum CorrectionWatcher {
    /// Délai laissé à l'utilisateur pour corriger avant la relecture.
    private static let delay: TimeInterval = 12

    static func watch(inserted text: String, in target: AccessibilityTarget?) {
        guard let target, VocabularyStore.shared.learnCorrections else { return }
        let inserted = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard inserted.split(separator: " ").count <= 60 else { return }

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(delay))
            guard let current = target.readValue() else { return }
            compare(inserted: inserted, current: current)
        }
    }

    private static func compare(inserted: String, current: String) {
        // Le texte inséré doit encore être identifiable dans le champ.
        guard !current.contains(inserted) else { return }

        let before = inserted.split(separator: " ").map(String.init)
        let after = current.split(separator: " ").map(String.init)
        guard before.count == after.count else { return }

        for (old, new) in zip(before, after) where old != new {
            let cleanOld = old.trimmingCharacters(in: .punctuationCharacters)
            let cleanNew = new.trimmingCharacters(in: .punctuationCharacters)
            // Substitution plausible : longueurs proches, pas une phrase entière.
            guard abs(cleanOld.count - cleanNew.count) <= max(4, cleanOld.count / 2) else { continue }
            VocabularyStore.shared.learn(heard: cleanOld, replacement: cleanNew)
        }
    }
}
