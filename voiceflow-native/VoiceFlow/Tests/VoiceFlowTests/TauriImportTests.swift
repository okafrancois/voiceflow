import Foundation
import Testing
@testable import VoiceFlow

struct TauriImportTests {
    @Test func pipeEntriesUseTheFirstTermAsTheSpellingToWrite() {
        let entries = TauriImport.parseDictionary("AES-GCM | AES GCM | A E S GCM\nANEF | a n e f")
        #expect(entries.map(\.replacement) == ["AES-GCM", "ANEF"])
        #expect(entries[0].heard == "AES GCM")
        #expect(entries[0].variants == ["A E S GCM"])
    }

    @Test func arrowEntriesMapWrongToRight() {
        let entries = TauriImport.parseDictionary("kubernetis -> Kubernetes; clode → Claude")
        #expect(entries.map { "\($0.heard)=\($0.replacement)" } == ["kubernetis=Kubernetes", "clode=Claude"])
    }

    @Test func aBareTermWithoutAliasIsSkipped() {
        // A bare term replaces nothing; it only hints the engine.
        #expect(TauriImport.parseDictionary("Anthropic").isEmpty)
        #expect(TauriImport.parseHints("Anthropic, Kubernetes | kube") == ["Anthropic", "Kubernetes"])
    }

    @Test func hintEntriesNeverChangeTheCaseOfOrdinaryWords() {
        let hint = TauriImport.hintEntry("Go")
        let result = VocabularyMatcher.apply(entries: [hint], snippets: [], to: "let's go, Go is fast")
        #expect(result.text == "let's go, Go is fast")
    }
}
