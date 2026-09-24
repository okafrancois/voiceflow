import AppKit
import Foundation

/// Apprentissage des corrections : après une insertion, on relit le champ
/// cible et on compare mot à mot avec ce qui avait été écrit. Les mots que
/// l'utilisateur a remplacés deviennent des suggestions de dictionnaire.
///
/// Volontairement prudent : une seule relecture différée, uniquement sur des
/// substitutions mot pour mot de longueur comparable. Tout le reste (phrases
/// réécrites, texte effacé, ajouts) est ignoré, car on ne saurait pas en
/// déduire une correction fiable. N'est lancé que si le texte a bien été
/// écrit dans le champ surveillé.
@MainActor
enum CorrectionWatcher {
    /// Délai laissé à l'utilisateur pour corriger avant la relecture.
    private static let delay: TimeInterval = 12

    static func watch(inserted text: String, in target: AccessibilityTarget) {
        guard VocabularyStore.shared.learnCorrections else { return }
        let inserted = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard inserted.split(separator: " ").count <= 60 else { return }

        Task { @MainActor in
            try? await Task.sleep(for: .seconds(delay))
            guard let current = await AXQueue.run({ target.readValue() }) else { return }
            for pair in corrections(inserted: inserted, current: current) {
                VocabularyStore.shared.learn(heard: pair.heard, replacement: pair.replacement)
            }
        }
    }

    /// Paires « écrit → corrigé » plausibles entre le texte inséré et le
    /// contenu actuel du champ.
    nonisolated static func corrections(
        inserted: String, current: String
    ) -> [(heard: String, replacement: String)] {
        // Le texte inséré doit avoir changé, mais rester reconnaissable.
        guard !current.contains(inserted) else { return [] }

        let before = inserted.split(separator: " ").map(String.init)
        let after = current.split(separator: " ").map(String.init)
        guard before.count == after.count else { return [] }

        var pairs: [(heard: String, replacement: String)] = []
        for (old, new) in zip(before, after) where old != new {
            let cleanOld = old.trimmingCharacters(in: .punctuationCharacters)
            let cleanNew = new.trimmingCharacters(in: .punctuationCharacters)
            // Substitution plausible : longueurs proches, pas une phrase entière.
            guard abs(cleanOld.count - cleanNew.count) <= max(4, cleanOld.count / 2) else { continue }
            pairs.append((cleanOld, cleanNew))
        }
        // Plus d'un mot sur trois changé : c'est une réécriture, pas une
        // correction de transcription.
        guard pairs.count * 3 <= before.count else { return [] }
        return pairs
    }
}
