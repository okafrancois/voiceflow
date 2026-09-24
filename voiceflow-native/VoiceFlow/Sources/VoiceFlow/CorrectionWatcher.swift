import AppKit
import Foundation

/// Correction learning: after an insertion, the target field is re-read
/// and compared word by word with what was written. Words the user
/// replaced become dictionary suggestions.
///
/// Deliberately cautious: a single deferred re-read, only for word-for-word
/// substitutions of comparable length. Everything else (rewritten sentences,
/// erased text, additions) is ignored, since no reliable correction could
/// be inferred from it. Only runs if the text was indeed written into the
/// watched field.
@MainActor
enum CorrectionWatcher {
    /// Delay given to the user to correct before the re-read.
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

    /// Plausible "written → corrected" pairs between the inserted text
    /// and the field's current content.
    nonisolated static func corrections(
        inserted: String, current: String
    ) -> [(heard: String, replacement: String)] {
        // The inserted text must have changed, but remain recognizable.
        guard !current.contains(inserted) else { return [] }

        let before = inserted.split(separator: " ").map(String.init)
        let after = current.split(separator: " ").map(String.init)
        guard before.count == after.count else { return [] }

        var pairs: [(heard: String, replacement: String)] = []
        for (old, new) in zip(before, after) where old != new {
            let cleanOld = old.trimmingCharacters(in: .punctuationCharacters)
            let cleanNew = new.trimmingCharacters(in: .punctuationCharacters)
            // Plausible substitution: similar lengths, not a whole sentence.
            guard abs(cleanOld.count - cleanNew.count) <= max(4, cleanOld.count / 2) else { continue }
            pairs.append((cleanOld, cleanNew))
        }
        // More than one word in three changed: that's a rewrite, not a
        // transcription correction.
        guard pairs.count * 3 <= before.count else { return [] }
        return pairs
    }
}
