import Foundation
import Testing
@testable import VoiceFlow

struct PolishChunkerTests {
    private func words(_ count: Int, prefix: String = "mot") -> String {
        (1...count).map { "\(prefix)\($0)" }.joined(separator: " ")
    }

    @Test func shortTextIsASingleChunk() {
        let chunks = PolishChunker.chunks(of: "Une phrase courte.", maxWords: 50)
        #expect(chunks.map(\.text) == ["Une phrase courte."])
    }

    @Test func paragraphsAreKeptTogetherWhenTheyFit() {
        let text = "Premier paragraphe.\n\nSecond paragraphe."
        let chunks = PolishChunker.chunks(of: text, maxWords: 50)
        #expect(chunks.count == 1)
        #expect(PolishChunker.join(chunks.map(\.text), like: chunks) == text)
    }

    @Test func longTextIsSplitOnSentencesAndRejoinedLosslessly() {
        let sentence = words(20) + "."
        let text = Array(repeating: sentence, count: 6).joined(separator: " ")
        let chunks = PolishChunker.chunks(of: text, maxWords: 50)
        #expect(chunks.count == 3)
        #expect(chunks.allSatisfy { PolishGuard.wordCount($0.text) <= 50 })
        #expect(PolishChunker.join(chunks.map(\.text), like: chunks) == text)
    }

    @Test func paragraphBoundariesSurviveTheSplit() {
        let paragraph = Array(repeating: words(20) + ".", count: 3).joined(separator: " ")
        let text = paragraph + "\n\n" + paragraph
        let chunks = PolishChunker.chunks(of: text, maxWords: 50)
        #expect(PolishChunker.join(chunks.map(\.text), like: chunks) == text)
    }
}

struct PolishGuardTests {
    @Test func shortDictationsAreNeverJudgedOnLength() {
        #expect(!PolishGuard.destroysContent(raw: "euh bon voilà", polished: "Bon.", templateID: "filler"))
    }

    @Test func anAnswerInsteadOfARewriteIsCaught() {
        let raw = "est-ce que tu peux vérifier que le serveur de production répond bien depuis ce matin et me dire"
        #expect(PolishGuard.destroysContent(raw: raw, polished: "Je ne peux pas vérifier cela.", templateID: "filler"))
    }

    @Test func conciseStyleMayShortenMore() {
        let raw = Array(repeating: "mot", count: 20).joined(separator: " ")
        let half = Array(repeating: "mot", count: 10).joined(separator: " ")
        #expect(!PolishGuard.destroysContent(raw: raw, polished: half, templateID: "concise"))
        #expect(PolishGuard.destroysContent(raw: raw, polished: half, templateID: "filler"))
    }
}

struct SilenceTrimmerTests {
    private let t0 = Date(timeIntervalSinceReferenceDate: 0)
    private func at(_ seconds: TimeInterval) -> Date { t0.addingTimeInterval(seconds) }

    /// Replays (level, time) blocks and returns the decisions.
    private func run(_ trimmer: inout SilenceTrimmer, _ blocks: [(Float, TimeInterval)]) -> [Bool] {
        blocks.map { trimmer.shouldPass(level: $0.0, now: at($0.1)) }
    }

    /// The first block, judged before any calibration, passes: letting
    /// silence through costs nothing, cutting speech costs words.
    @Test func silenceIsCutOnlyAfterTheHangoverAndSpeechResumes() {
        var trimmer = SilenceTrimmer(margin: 0.05, hangover: 1.2, keptPause: 0.6)
        let decisions = run(&trimmer, [(0.1, 0), (0.1, 0.5), (0.1, 1.3), (0.1, 2.0), (0.6, 2.2)])
        #expect(decisions == [true, true, true, false, true])
    }

    @Test func aShortPauseInsideASentenceIsNeverCut() {
        var trimmer = SilenceTrimmer(margin: 0.05, hangover: 1.2)
        #expect(run(&trimmer, [(0.1, 0), (0.6, 0.1), (0.1, 1.0)]).last == true)
    }

    @Test func aLongPauseIsShortened() {
        var trimmer = SilenceTrimmer(margin: 0.05, hangover: 1.2, keptPause: 0.6)
        #expect(run(&trimmer, [(0.1, 0), (0.6, 0.1), (0.1, 1.5), (0.1, 2.5)]).suffix(2) == [true, false])
    }

    @Test func aMicTooQuietToCalibrateFailsOpen() {
        var trimmer = SilenceTrimmer(margin: 0.05, failOpenAfter: 1.5)
        #expect(run(&trimmer, [(0.01, 0), (0.01, 1.0), (0.01, 1.6)]) == [false, false, true])
    }
}
