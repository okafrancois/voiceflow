import Foundation
import Testing
@testable import VoiceFlow

struct VocabularyMatcherTests {
    private func entry(_ heard: String, _ replacement: String, variants: [String] = []) -> DictionaryEntry {
        DictionaryEntry(heard: heard, variants: variants, replacement: replacement)
    }

    @Test func replacesWholeWordsOnly() {
        let result = VocabularyMatcher.apply(
            entries: [entry("ia", "IA")], snippets: [], to: "La confiance dans l'ia grandit")
        #expect(result.text == "La confiance dans l'IA grandit")
    }

    @Test func aCorrectionNeverEatsIntoALongerWord() {
        let result = VocabularyMatcher.apply(
            entries: [entry("sur", "sûr")], snippets: [], to: "Surtout, je suis sur de moi")
        #expect(result.text == "Surtout, je suis sûr de moi")
    }

    @Test func matchingIgnoresCaseButNotAccents() {
        let loose = VocabularyMatcher.apply(
            entries: [entry("kubernetis", "Kubernetes")], snippets: [], to: "Déployé sur KUBERNETIS hier")
        #expect(loose.text == "Déployé sur Kubernetes hier")

        // "peche → pêche" must not rewrite "péché".
        let accents = VocabularyMatcher.apply(
            entries: [entry("peche", "pêche")], snippets: [], to: "un péché, la peche")
        #expect(accents.text == "un péché, la pêche")

        var strict = entry("Go", "Golang")
        strict.caseSensitive = true
        let result = VocabularyMatcher.apply(entries: [strict], snippets: [], to: "go Go")
        #expect(result.text == "go Golang")
    }

    @Test func variantsAreReplacedLongestFirst() {
        let result = VocabularyMatcher.apply(
            entries: [entry("open ai", "OpenAI", variants: ["open a i"])],
            snippets: [], to: "chez open a i et open ai")
        #expect(result.text == "chez OpenAI et OpenAI")
    }

    @Test func termsWithSymbolsStillMatch() {
        let result = VocabularyMatcher.apply(
            entries: [entry("c plus plus", "C++")], snippets: [], to: "En c plus plus, bien sûr")
        #expect(result.text == "En C++, bien sûr")
    }

    @Test func suggestionsAreNotAppliedUntilAccepted() {
        var pending = entry("claud", "Claude")
        pending.pendingSightings = 1
        let result = VocabularyMatcher.apply(entries: [pending], snippets: [], to: "demande à claud")
        #expect(result.text == "demande à claud")
        #expect(result.usedEntries.isEmpty)
    }

    @Test func snippetsExpandOnWholePhrasesAndReportUse() {
        let snippet = Snippet(trigger: "ma signature", expansion: "Berny — Okatech")
        let result = VocabularyMatcher.apply(
            entries: [], snippets: [snippet], to: "Merci, ma signature")
        #expect(result.text == "Merci, Berny — Okatech")
        #expect(result.usedSnippets == [snippet.id])

        let inside = VocabularyMatcher.apply(
            entries: [], snippets: [snippet], to: "voir ma signatures")
        #expect(inside.text == "voir ma signatures")
    }

    @Test func legacyEntriesWithoutNewFieldsStillDecode() throws {
        let json = #"[{"id":"6F9619FF-8B86-D011-B42D-00CF4FC964FF","heard":"a","variants":[],"replacement":"b","caseSensitive":false,"useCount":2,"learned":true}]"#
        let entries = try JSONDecoder().decode([DictionaryEntry].self, from: Data(json.utf8))
        #expect(entries.count == 1)
        #expect(entries[0].isActive)
        #expect(entries[0].useCount == 2)
    }
}

struct CorrectionDiffTests {
    @Test func aReplacedWordIsReported() {
        let pairs = CorrectionWatcher.corrections(
            inserted: "Déployé sur kubernetis hier", current: "Déployé sur Kubernetes hier")
        #expect(pairs.map { "\($0.heard)→\($0.replacement)" } == ["kubernetis→Kubernetes"])
    }

    @Test func rewrittenSentencesTeachNothing() {
        #expect(CorrectionWatcher.corrections(
            inserted: "on se voit vendredi", current: "rendez-vous samedi prochain à midi").isEmpty)
    }

    @Test func untouchedTextTeachesNothing() {
        #expect(CorrectionWatcher.corrections(inserted: "bonjour", current: "bonjour").isEmpty)
    }
}
