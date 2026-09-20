import Foundation

/// Un terme du dictionnaire : ce que la transcription produit → ce qu'il faut
/// écrire. Appliqué après la transcription, avant le polissage.
struct DictionaryEntry: Codable, Identifiable, Hashable {
    var id = UUID()
    /// Forme entendue principale.
    var heard: String
    /// Autres graphies produites par la transcription pour le même mot.
    var variants: [String] = []
    var replacement: String
    var caseSensitive = false
    var useCount = 0
    var lastUsed: Date?
    /// Vrai quand l'entrée vient d'une correction observée, pas d'une saisie.
    var learned = false

    /// Toutes les formes à remplacer, la plus longue d'abord pour éviter
    /// qu'une forme courte n'entame une forme longue.
    var allForms: [String] {
        ([heard] + variants)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
            .sorted { $0.count > $1.count }
    }
}

/// Un extrait : une phrase dictée qui se remplace par un texte plus long.
struct Snippet: Codable, Identifiable, Hashable {
    var id = UUID()
    var trigger: String
    var expansion: String
    var useCount = 0
}

/// Une règle d'application : quel style de polissage utiliser selon l'app
/// dans laquelle on dicte.
struct AppRule: Codable, Identifiable, Hashable {
    var id = UUID()
    var bundleID: String
    var appName: String
    var templateID: String
}

/// Dictionnaire, extraits et règles d'application, dans un simple JSON
/// à côté de la base d'historique.
@MainActor
final class VocabularyStore: ObservableObject {
    static let shared = VocabularyStore()

    @Published var entries: [DictionaryEntry] = [] { didSet { save() } }
    @Published var snippets: [Snippet] = [] { didSet { save() } }
    @Published var appRules: [AppRule] = [] { didSet { save() } }

    /// Prompts de polissage modifiés par l'utilisateur, par identifiant de
    /// style. Absent = prompt d'origine.
    @Published var customPrompts: [String: String] = [:] { didSet { save() } }

    /// Apprendre automatiquement les corrections faites après insertion.
    @Published var learnCorrections = UserDefaults.standard.object(forKey: "learnCorrections") as? Bool ?? true {
        didSet { UserDefaults.standard.set(learnCorrections, forKey: "learnCorrections") }
    }

    private struct Payload: Codable {
        var entries: [DictionaryEntry] = []
        var snippets: [Snippet] = []
        var appRules: [AppRule] = []
        var customPrompts: [String: String] = [:]
    }

    private let url: URL
    private var loading = false

    private init() {
        let directory = URL.applicationSupportDirectory.appending(path: "VoiceFlow")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        url = directory.appending(path: "vocabulary.json")
        load()
    }

    private func load() {
        guard let data = try? Data(contentsOf: url),
              let payload = try? JSONDecoder().decode(Payload.self, from: data)
        else { return }
        loading = true
        entries = payload.entries
        snippets = payload.snippets
        appRules = payload.appRules
        customPrompts = payload.customPrompts.mapValues {
            PolishCatalog.upgraded($0) ?? $0
        }
        loading = false
        if customPrompts != payload.customPrompts { save() }
    }

    private func save() {
        guard !loading else { return }
        let payload = Payload(
            entries: entries, snippets: snippets, appRules: appRules,
            customPrompts: customPrompts)
        guard let data = try? JSONEncoder().encode(payload) else { return }
        try? data.write(to: url, options: .atomic)
    }

    /// Applique les extraits puis le dictionnaire au texte transcrit.
    /// Les extraits d'abord : ils peuvent produire du texte que le
    /// dictionnaire corrigera ensuite.
    func apply(to text: String) -> String {
        var result = text

        for index in snippets.indices {
            let trigger = snippets[index].trigger.trimmingCharacters(in: .whitespaces)
            guard !trigger.isEmpty, result.localizedCaseInsensitiveContains(trigger) else { continue }
            result = result.replacingOccurrences(
                of: trigger, with: snippets[index].expansion,
                options: [.caseInsensitive, .diacriticInsensitive])
            snippets[index].useCount += 1
        }

        for index in entries.indices {
            let options: String.CompareOptions = entries[index].caseSensitive
                ? [] : [.caseInsensitive, .diacriticInsensitive]
            var used = false
            for form in entries[index].allForms {
                guard result.range(of: form, options: options) != nil else { continue }
                result = result.replacingOccurrences(
                    of: form, with: entries[index].replacement, options: options)
                used = true
            }
            if used {
                entries[index].useCount += 1
                entries[index].lastUsed = Date()
            }
        }

        return result
    }

    /// Importe un CSV « entendu,correction » (séparateur virgule ou
    /// point-virgule, une paire par ligne). Renvoie le nombre d'entrées
    /// ajoutées.
    @discardableResult
    func importCSV(from url: URL) throws -> Int {
        let text = try String(contentsOf: url, encoding: .utf8)
        var added = 0
        for line in text.split(whereSeparator: \.isNewline) {
            let parts = line.split(whereSeparator: { $0 == "," || $0 == ";" })
            guard parts.count >= 2 else { continue }
            let heard = parts[0].trimmingCharacters(in: .whitespaces)
                .trimmingCharacters(in: CharacterSet(charactersIn: "\""))
            let replacement = parts[1].trimmingCharacters(in: .whitespaces)
                .trimmingCharacters(in: CharacterSet(charactersIn: "\""))
            guard !heard.isEmpty, !replacement.isEmpty,
                  !entries.contains(where: { $0.heard.caseInsensitiveCompare(heard) == .orderedSame })
            else { continue }
            entries.append(DictionaryEntry(heard: heard, replacement: replacement))
            added += 1
        }
        return added
    }

    /// Enregistre une correction observée : le mot inséré a été remplacé par
    /// un autre dans le champ cible.
    func learn(heard: String, replacement: String) {
        let heard = heard.trimmingCharacters(in: .whitespaces)
        let replacement = replacement.trimmingCharacters(in: .whitespaces)
        guard heard.count > 2, replacement.count > 1,
              heard.caseInsensitiveCompare(replacement) != .orderedSame
        else { return }

        if let index = entries.firstIndex(where: {
            $0.replacement.caseInsensitiveCompare(replacement) == .orderedSame
        }) {
            // Même correction, nouvelle graphie entendue : ajouter la variante.
            guard !entries[index].allForms.contains(where: {
                $0.caseInsensitiveCompare(heard) == .orderedSame
            }) else { return }
            entries[index].variants.append(heard)
        } else {
            entries.append(DictionaryEntry(
                heard: heard, replacement: replacement, learned: true))
        }
        log.info("learned correction: \(heard) → \(replacement)")
    }

    /// Style de polissage à utiliser pour une application donnée, s'il existe
    /// une règle.
    func templateID(forBundleID bundleID: String?) -> String? {
        guard let bundleID else { return nil }
        return appRules.first { $0.bundleID == bundleID }?.templateID
    }
}
