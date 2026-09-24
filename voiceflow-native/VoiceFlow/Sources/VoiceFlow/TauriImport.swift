import Foundation

/// Reprise des données de l'app Tauri : historique, dictionnaire, extraits.
///
/// Le dictionnaire Tauri tient dans un seul texte (`custom_dictionary` des
/// réglages) : une entrée par ligne, virgule ou point-virgule, sous la forme
/// `terme | variante | variante` ou `entendu -> terme`
/// (`src-tauri/src/correction_learning/hotwords.rs`).
enum TauriImport {
    static var dataDirectory: URL {
        URL.applicationSupportDirectory.appending(path: "com.voiceflow.voicetotext")
    }

    static var historyURL: URL { dataDirectory.appending(path: "transcription_history.db") }
    static var settingsURL: URL { dataDirectory.appending(path: "settings.json") }

    static var isAvailable: Bool {
        FileManager.default.fileExists(atPath: historyURL.path(percentEncoded: false))
            || FileManager.default.fileExists(atPath: settingsURL.path(percentEncoded: false))
    }

    private static func entries(of content: String) -> [String] {
        content.split(whereSeparator: { "\n\r,，;；".contains($0) })
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    /// Entrées qui remplacent quelque chose : un terme avec au moins une
    /// variante, ou une flèche.
    static func parseDictionary(_ content: String) -> [DictionaryEntry] {
        entries(of: content).compactMap { line in
            for arrow in ["->", "=>", "→"] {
                if let range = line.range(of: arrow) {
                    let heard = line[..<range.lowerBound].trimmingCharacters(in: .whitespaces)
                    let term = line[range.upperBound...].trimmingCharacters(in: .whitespaces)
                    guard !heard.isEmpty, !term.isEmpty else { return nil }
                    return DictionaryEntry(heard: heard, replacement: term)
                }
            }
            let parts = line.split(separator: "|")
                .map { $0.trimmingCharacters(in: .whitespaces) }
                .filter { !$0.isEmpty }
            guard parts.count >= 2 else { return nil }
            let aliases = parts.dropFirst().filter { $0.caseInsensitiveCompare(parts[0]) != .orderedSame }
            guard let heard = aliases.first else { return nil }
            return DictionaryEntry(heard: heard, variants: Array(aliases.dropFirst()), replacement: parts[0])
        }
    }

    /// Terme seul importé : sensible à la casse et identique à lui-même, il
    /// ne réécrit jamais rien (« Go » laisse « let's go » tranquille) mais
    /// figure parmi les indices donnés au moteur.
    static func hintEntry(_ term: String) -> DictionaryEntry {
        DictionaryEntry(heard: term, replacement: term, caseSensitive: true)
    }

    /// Termes seuls : rien à remplacer, mais le moteur doit les connaître.
    static func parseHints(_ content: String) -> [String] {
        entries(of: content).compactMap { line in
            guard !["->", "=>", "→"].contains(where: line.contains) else { return nil }
            return line.split(separator: "|").first.map { $0.trimmingCharacters(in: .whitespaces) }
        }
    }

    struct Report {
        var history = 0
        var dictionary = 0
        var snippets = 0
    }

    private struct Settings: Decodable {
        struct VoiceSnippet: Decodable {
            let spoken_trigger: String
            let template: String
            let enabled: Bool?
        }
        let custom_dictionary: String?
        let voice_snippets: [VoiceSnippet]?
    }

    /// Importe tout ce qui est disponible. Relançable : rien n'est ajouté
    /// deux fois.
    @MainActor
    static func importAll() throws -> Report {
        var report = Report()
        let historyPath = historyURL.path(percentEncoded: false)
        if FileManager.default.fileExists(atPath: historyPath) {
            report.history = try HistoryStore.shared.importTauriHistory(from: historyPath)
        }

        guard let data = try? Data(contentsOf: settingsURL) else { return report }
        let settings = try JSONDecoder().decode(Settings.self, from: data)
        let store = VocabularyStore.shared

        if let dictionary = settings.custom_dictionary {
            var entries = store.entries
            let known = Set(entries.map { $0.replacement.lowercased() })
            for entry in parseDictionary(dictionary) where !known.contains(entry.replacement.lowercased()) {
                entries.append(entry)
                report.dictionary += 1
            }
            // Les termes seuls deviennent des entrées sans variante : ils
            // n'écrivent rien de plus, mais nourrissent la reconnaissance.
            let withEntries = Set(entries.map { $0.replacement.lowercased() })
            for hint in parseHints(dictionary) where !withEntries.contains(hint.lowercased()) {
                entries.append(hintEntry(hint))
                report.dictionary += 1
            }
            store.entries = entries
        }

        for snippet in settings.voice_snippets ?? [] where snippet.enabled ?? true {
            guard !store.snippets.contains(where: {
                $0.trigger.caseInsensitiveCompare(snippet.spoken_trigger) == .orderedSame
            }) else { continue }
            store.snippets.append(Snippet(trigger: snippet.spoken_trigger, expansion: snippet.template))
            report.snippets += 1
        }
        return report
    }
}
