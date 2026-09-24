import AppKit
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
    /// Correction observée mais pas encore validée : nombre de fois où elle
    /// a été vue. `nil` = entrée active. Une correction isolée peut être un
    /// changement d'avis (« vendredi » → « samedi ») et non une erreur de
    /// transcription : elle ne s'applique qu'une fois acceptée, ou revue
    /// plusieurs fois.
    var pendingSightings: Int?

    var isActive: Bool { pendingSightings == nil }

    /// Toutes les formes à remplacer, la plus longue d'abord pour éviter
    /// qu'une forme courte n'entame une forme longue.
    var allForms: [String] {
        ([heard] + variants)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
            .sorted { $0.count > $1.count }
    }

    init(heard: String, variants: [String] = [], replacement: String,
         caseSensitive: Bool = false, learned: Bool = false, pendingSightings: Int? = nil) {
        self.heard = heard
        self.variants = variants
        self.replacement = replacement
        self.caseSensitive = caseSensitive
        self.learned = learned
        self.pendingSightings = pendingSightings
    }

    /// Décodage tolérant : un champ absent (fichier d'une version plus
    /// ancienne) prend sa valeur par défaut au lieu de faire échouer tout le
    /// fichier — ce qui l'aurait vidé à la sauvegarde suivante.
    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(UUID.self, forKey: .id) ?? UUID()
        heard = try values.decode(String.self, forKey: .heard)
        variants = try values.decodeIfPresent([String].self, forKey: .variants) ?? []
        replacement = try values.decode(String.self, forKey: .replacement)
        caseSensitive = try values.decodeIfPresent(Bool.self, forKey: .caseSensitive) ?? false
        useCount = try values.decodeIfPresent(Int.self, forKey: .useCount) ?? 0
        lastUsed = try values.decodeIfPresent(Date.self, forKey: .lastUsed)
        learned = try values.decodeIfPresent(Bool.self, forKey: .learned) ?? false
        pendingSightings = try values.decodeIfPresent(Int.self, forKey: .pendingSightings)
    }
}

/// Un extrait : une phrase dictée qui se remplace par un texte plus long.
struct Snippet: Codable, Identifiable, Hashable {
    var id = UUID()
    var trigger: String
    var expansion: String
    var useCount = 0

    init(trigger: String, expansion: String) {
        self.trigger = trigger
        self.expansion = expansion
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(UUID.self, forKey: .id) ?? UUID()
        trigger = try values.decode(String.self, forKey: .trigger)
        expansion = try values.decode(String.self, forKey: .expansion)
        useCount = try values.decodeIfPresent(Int.self, forKey: .useCount) ?? 0
    }
}

/// Une règle d'application : style de polissage et langue de dictée à
/// utiliser selon l'app dans laquelle on dicte. `nil` = réglage général.
struct AppRule: Codable, Identifiable, Hashable {
    var id = UUID()
    var bundleID: String
    var appName: String
    var templateID: String?
    var localeID: String?
}

/// Remplacements du dictionnaire et des extraits, sans état ni fichier.
///
/// Un terme ne remplace que des mots entiers : en sous-chaîne, « ia → IA »
/// écrivait « confIAnce », et une correction apprise « sur → sûr » donnait
/// « sûrtout ».
enum VocabularyMatcher {
    struct Result {
        var text: String
        var usedEntries: Set<UUID> = []
        var usedSnippets: Set<UUID> = []
    }

    /// Les extraits d'abord : ils peuvent produire du texte que le
    /// dictionnaire corrigera ensuite.
    static func apply(entries: [DictionaryEntry], snippets: [Snippet], to text: String) -> Result {
        var result = Result(text: text)

        for snippet in snippets {
            let trigger = snippet.trigger.trimmingCharacters(in: .whitespaces)
            guard !trigger.isEmpty else { continue }
            let (replaced, count) = replacingWholeWords(
                trigger, with: snippet.expansion, in: result.text,
                options: [.caseInsensitive, .diacriticInsensitive])
            if count > 0 {
                result.text = replaced
                result.usedSnippets.insert(snippet.id)
            }
        }

        for entry in entries where entry.isActive {
            // Accents toujours significatifs : « peche → pêche » ne doit pas
            // réécrire « péché ». Une graphie accentuée différente s'ajoute
            // comme variante.
            let options: String.CompareOptions = entry.caseSensitive ? [] : [.caseInsensitive]
            for form in entry.allForms {
                let (replaced, count) = replacingWholeWords(
                    form, with: entry.replacement, in: result.text, options: options)
                if count > 0 {
                    result.text = replaced
                    result.usedEntries.insert(entry.id)
                }
            }
        }
        return result
    }

    static func replacingWholeWords(
        _ form: String, with replacement: String, in text: String,
        options: String.CompareOptions
    ) -> (text: String, count: Int) {
        var output = ""
        var cursor = text.startIndex
        var searchFrom = text.startIndex
        var count = 0
        while searchFrom < text.endIndex,
              let range = text.range(of: form, options: options, range: searchFrom..<text.endIndex) {
            let startsWord = range.lowerBound == text.startIndex
                || !isWordCharacter(text[text.index(before: range.lowerBound)])
            let endsWord = range.upperBound == text.endIndex
                || !isWordCharacter(text[range.upperBound])
            if startsWord, endsWord {
                output += text[cursor..<range.lowerBound]
                output += replacement
                cursor = range.upperBound
                searchFrom = range.upperBound
                count += 1
            } else {
                searchFrom = text.index(after: range.lowerBound)
            }
        }
        output += text[cursor...]
        return (output, count)
    }

    private static func isWordCharacter(_ character: Character) -> Bool {
        character.isLetter || character.isNumber || character == "_"
    }
}

/// Dictionnaire, extraits et règles d'application, dans un simple JSON
/// à côté de la base d'historique.
@MainActor
final class VocabularyStore: ObservableObject {
    static let shared = VocabularyStore()

    /// Nombre d'observations d'une même correction avant qu'elle ne
    /// s'applique d'elle-même.
    static let sightingsToActivate = 3

    @Published var entries: [DictionaryEntry] = [] { didSet { scheduleSave() } }
    @Published var snippets: [Snippet] = [] { didSet { scheduleSave() } }
    @Published var appRules: [AppRule] = [] { didSet { scheduleSave() } }

    /// Prompts de polissage modifiés par l'utilisateur, par identifiant de
    /// style. Absent = prompt d'origine.
    @Published var customPrompts: [String: String] = [:] { didSet { scheduleSave() } }

    /// Apprendre automatiquement les corrections faites après insertion.
    @Published var learnCorrections = UserDefaults.standard.object(forKey: "learnCorrections") as? Bool ?? true {
        didSet { UserDefaults.standard.set(learnCorrections, forKey: "learnCorrections") }
    }

    /// Transmettre les termes du dictionnaire au moteur de transcription,
    /// pour qu'il les reconnaisse du premier coup.
    @Published var biasRecognition = UserDefaults.standard.object(forKey: "biasRecognition") as? Bool ?? true {
        didSet { UserDefaults.standard.set(biasRecognition, forKey: "biasRecognition") }
    }

    private struct Payload: Codable {
        var entries: [DictionaryEntry] = []
        var snippets: [Snippet] = []
        var appRules: [AppRule] = []
        var customPrompts: [String: String] = [:]

        init(entries: [DictionaryEntry], snippets: [Snippet], appRules: [AppRule],
             customPrompts: [String: String]) {
            self.entries = entries
            self.snippets = snippets
            self.appRules = appRules
            self.customPrompts = customPrompts
        }

        /// Une section absente (fichier plus ancien) n'invalide pas le reste.
        init(from decoder: Decoder) throws {
            let values = try decoder.container(keyedBy: CodingKeys.self)
            entries = try values.decodeIfPresent([DictionaryEntry].self, forKey: .entries) ?? []
            snippets = try values.decodeIfPresent([Snippet].self, forKey: .snippets) ?? []
            appRules = try values.decodeIfPresent([AppRule].self, forKey: .appRules) ?? []
            customPrompts = try values.decodeIfPresent([String: String].self, forKey: .customPrompts) ?? [:]
        }
    }

    private let url: URL
    private var loading = false
    private var saveTask: Task<Void, Never>?

    private init() {
        let directory = URL.applicationSupportDirectory.appending(path: "VoiceFlow")
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        url = directory.appending(path: "vocabulary.json")
        load()
        // La sauvegarde est différée : la forcer avant de quitter.
        NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification, object: nil, queue: .main
        ) { _ in
            MainActor.assumeIsolated { VocabularyStore.shared.flush() }
        }
    }

    /// Écrit tout de suite ce qui attendait la sauvegarde différée.
    func flush() {
        guard saveTask != nil else { return }
        saveTask?.cancel()
        saveTask = nil
        save()
    }

    private func load() {
        guard let data = try? Data(contentsOf: url) else { return }
        let payload: Payload
        do {
            payload = try JSONDecoder().decode(Payload.self, from: data)
        } catch {
            // Fichier illisible : le mettre de côté plutôt que de l'écraser
            // à la prochaine sauvegarde.
            let backup = url.deletingPathExtension()
                .appendingPathExtension("unreadable-\(Int(Date().timeIntervalSince1970)).json")
            try? FileManager.default.copyItem(at: url, to: backup)
            log.error("vocabulary unreadable, kept a copy at \(backup.path): \(error)")
            Diagnostics.log("dictionnaire illisible, copie conservée : \(backup.lastPathComponent)")
            return
        }
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

    /// Une dictée touche plusieurs compteurs d'usage : on regroupe les
    /// écritures au lieu d'en faire une par entrée modifiée.
    private func scheduleSave() {
        guard !loading else { return }
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            self?.saveTask = nil
            self?.save()
        }
    }

    func save() {
        let payload = Payload(
            entries: entries, snippets: snippets, appRules: appRules,
            customPrompts: customPrompts)
        do {
            let data = try JSONEncoder().encode(payload)
            try data.write(to: url, options: .atomic)
        } catch {
            log.error("vocabulary save failed: \(error)")
        }
    }

    /// Applique les extraits puis le dictionnaire au texte transcrit.
    func apply(to text: String) -> String {
        let result = VocabularyMatcher.apply(entries: entries, snippets: snippets, to: text)
        guard !result.usedEntries.isEmpty || !result.usedSnippets.isEmpty else { return result.text }
        let now = Date()
        var updatedEntries = entries
        for index in updatedEntries.indices where result.usedEntries.contains(updatedEntries[index].id) {
            updatedEntries[index].useCount += 1
            updatedEntries[index].lastUsed = now
        }
        var updatedSnippets = snippets
        for index in updatedSnippets.indices where result.usedSnippets.contains(updatedSnippets[index].id) {
            updatedSnippets[index].useCount += 1
        }
        entries = updatedEntries
        snippets = updatedSnippets
        return result.text
    }

    /// Les termes à signaler au moteur de transcription : les graphies
    /// voulues, les plus utilisées d'abord.
    var recognitionHints: [String] {
        guard biasRecognition else { return [] }
        let terms = entries.filter(\.isActive)
            .sorted { $0.useCount > $1.useCount }
            .map(\.replacement)
        var seen = Set<String>()
        return terms.filter { seen.insert($0.lowercased()).inserted }.prefix(64).map { $0 }
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
    /// un autre dans le champ cible. Elle reste une suggestion tant qu'elle
    /// n'a pas été acceptée ou revue plusieurs fois.
    func learn(heard: String, replacement: String) {
        let heard = heard.trimmingCharacters(in: .whitespaces)
        let replacement = replacement.trimmingCharacters(in: .whitespaces)
        guard heard.count > 2, replacement.count > 1,
              heard.caseInsensitiveCompare(replacement) != .orderedSame
        else { return }

        if let index = entries.firstIndex(where: {
            $0.replacement.caseInsensitiveCompare(replacement) == .orderedSame
        }) {
            var entry = entries[index]
            if entry.allForms.contains(where: { $0.caseInsensitiveCompare(heard) == .orderedSame }) {
                // Déjà connue : une observation de plus pour une suggestion.
                guard let sightings = entry.pendingSightings else { return }
                entry.pendingSightings = sightings + 1 >= Self.sightingsToActivate ? nil : sightings + 1
                entries[index] = entry
            } else if !entry.isActive {
                // Suggestion encore en attente : la nouvelle graphie la rejoint.
                entry.variants.append(heard)
                entries[index] = entry
            } else {
                // Entrée active : une graphie nouvelle ne s'applique pas
                // d'office, elle devient sa propre suggestion.
                entries.append(DictionaryEntry(
                    heard: heard, replacement: replacement, learned: true, pendingSightings: 1))
            }
        } else {
            entries.append(DictionaryEntry(
                heard: heard, replacement: replacement, learned: true, pendingSightings: 1))
        }
        log.info("learned correction: \(heard) → \(replacement)")
    }

    func accept(_ entry: DictionaryEntry) {
        guard let index = entries.firstIndex(where: { $0.id == entry.id }) else { return }
        entries[index].pendingSightings = nil
    }

    /// Style de polissage à utiliser pour une application donnée, s'il existe
    /// une règle.
    func templateID(forBundleID bundleID: String?) -> String? {
        rule(for: bundleID)?.templateID
    }

    /// Langue de dictée à utiliser pour une application donnée.
    func localeID(forBundleID bundleID: String?) -> String? {
        rule(for: bundleID)?.localeID
    }

    private func rule(for bundleID: String?) -> AppRule? {
        guard let bundleID else { return nil }
        return appRules.first { $0.bundleID == bundleID }
    }
}
