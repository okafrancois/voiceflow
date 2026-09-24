import AppKit
import Foundation

/// A dictionary term: what the transcription produces → what should
/// be written. Applied after transcription, before polishing.
struct DictionaryEntry: Codable, Identifiable, Hashable {
    var id = UUID()
    /// Main heard form.
    var heard: String
    /// Other spellings produced by the transcription for the same word.
    var variants: [String] = []
    var replacement: String
    var caseSensitive = false
    var useCount = 0
    var lastUsed: Date?
    /// True when the entry comes from an observed correction, not manual entry.
    var learned = false
    /// Correction observed but not yet confirmed: number of times it has
    /// been seen. `nil` = active entry. An isolated correction could be a
    /// change of mind (« vendredi » → « samedi ») rather than a
    /// transcription error: it only applies once accepted, or seen
    /// several times.
    var pendingSightings: Int?

    var isActive: Bool { pendingSightings == nil }

    /// All the forms to replace, longest first to avoid a short form
    /// cutting into a longer one.
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

    /// Tolerant decoding: a missing field (file from an older version)
    /// takes its default value instead of failing the whole file —
    /// which would have wiped it out on the next save.
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

/// A snippet: a dictated phrase that gets replaced by a longer text.
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

/// An app rule: polish style and dictation language to use depending
/// on the app being dictated into. `nil` = general setting.
struct AppRule: Codable, Identifiable, Hashable {
    var id = UUID()
    var bundleID: String
    var appName: String
    var templateID: String?
    var localeID: String?
}

/// Dictionary and snippet replacements, with no state and no file.
///
/// A term only replaces whole words: as a substring, « ia → IA »
/// would write « confIAnce », and a learned correction « sur → sûr » would
/// give « sûrtout ».
enum VocabularyMatcher {
    struct Result {
        var text: String
        var usedEntries: Set<UUID> = []
        var usedSnippets: Set<UUID> = []
    }

    /// Snippets first: they can produce text that the dictionary will
    /// then correct.
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
            // Accents always matter: « peche → pêche » must not
            // rewrite « péché ». A differently accented spelling is added
            // as a variant.
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

/// Dictionary, snippets, and app rules, in a plain JSON file
/// next to the history database.
@MainActor
final class VocabularyStore: ObservableObject {
    static let shared = VocabularyStore()

    /// Number of times the same correction must be observed before it
    /// applies automatically.
    static let sightingsToActivate = 3

    @Published var entries: [DictionaryEntry] = [] { didSet { scheduleSave() } }
    @Published var snippets: [Snippet] = [] { didSet { scheduleSave() } }
    @Published var appRules: [AppRule] = [] { didSet { scheduleSave() } }

    /// Polish prompts modified by the user, keyed by style identifier.
    /// Absent = original prompt.
    @Published var customPrompts: [String: String] = [:] { didSet { scheduleSave() } }

    /// Automatically learn corrections made after insertion.
    @Published var learnCorrections = UserDefaults.standard.object(forKey: "learnCorrections") as? Bool ?? true {
        didSet { UserDefaults.standard.set(learnCorrections, forKey: "learnCorrections") }
    }

    /// Pass dictionary terms to the transcription engine, so it
    /// recognizes them right away.
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

        /// A missing section (older file) doesn't invalidate the rest.
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
        // Saving is deferred: force it before quitting.
        NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification, object: nil, queue: .main
        ) { _ in
            MainActor.assumeIsolated { VocabularyStore.shared.flush() }
        }
    }

    /// Writes immediately whatever was waiting for the deferred save.
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
            // Unreadable file: set it aside instead of overwriting it
            // on the next save.
            let backup = url.deletingPathExtension()
                .appendingPathExtension("unreadable-\(Int(Date().timeIntervalSince1970)).json")
            try? FileManager.default.copyItem(at: url, to: backup)
            log.error("vocabulary unreadable, kept a copy at \(backup.path): \(error)")
            Diagnostics.log("dictionary unreadable, kept a copy: \(backup.lastPathComponent)")
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

    /// A dictation touches several usage counters: writes are batched
    /// instead of one per modified entry.
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

    /// Applies snippets then the dictionary to the transcribed text.
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

    /// Terms to report to the transcription engine: the intended
    /// spellings, most used first.
    var recognitionHints: [String] {
        guard biasRecognition else { return [] }
        let terms = entries.filter(\.isActive)
            .sorted { $0.useCount > $1.useCount }
            .map(\.replacement)
        var seen = Set<String>()
        return terms.filter { seen.insert($0.lowercased()).inserted }.prefix(64).map { $0 }
    }

    /// Imports a "heard,correction" CSV (comma or semicolon separator,
    /// one pair per line). Returns the number of entries added.
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

    /// Records an observed correction: the inserted word was replaced by
    /// another one in the target field. It remains a suggestion until
    /// it has been accepted or seen several times.
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
                // Already known: one more sighting for a suggestion.
                guard let sightings = entry.pendingSightings else { return }
                entry.pendingSightings = sightings + 1 >= Self.sightingsToActivate ? nil : sightings + 1
                entries[index] = entry
            } else if !entry.isActive {
                // Suggestion still pending: the new spelling joins it.
                entry.variants.append(heard)
                entries[index] = entry
            } else {
                // Active entry: a new spelling doesn't apply
                // automatically, it becomes its own suggestion.
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

    /// Polish style to use for a given application, if a rule exists.
    func templateID(forBundleID bundleID: String?) -> String? {
        rule(for: bundleID)?.templateID
    }

    /// Dictation language to use for a given application.
    func localeID(forBundleID bundleID: String?) -> String? {
        rule(for: bundleID)?.localeID
    }

    private func rule(for bundleID: String?) -> AppRule? {
        guard let bundleID else { return nil }
        return appRules.first { $0.bundleID == bundleID }
    }
}
