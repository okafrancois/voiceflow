import Foundation
import Testing
@testable import VoiceFlow

struct SmartSpacingTests {
    @Test func unknownContextLeavesTheTextAlone() {
        #expect(SmartSpacing.adjust("Bonjour.", after: nil) == "Bonjour.")
        #expect(SmartSpacing.adjust("Bonjour.", after: "") == "Bonjour.")
    }

    @Test func aNewSentenceAfterAFullStopGetsASpace() {
        #expect(SmartSpacing.adjust("Comment ça va ?", after: "Bonjour.") == " Comment ça va ?")
    }

    @Test func noDoubleSpaceAfterExistingWhitespace() {
        #expect(SmartSpacing.adjust("Et toi ?", after: "Ça va. ") == "Et toi ?")
    }

    @Test func midSentenceContinuationIsLowercased() {
        #expect(SmartSpacing.adjust("Comment ça va", after: "Salut ") == "comment ça va")
        #expect(SmartSpacing.adjust("Et ensuite", after: "je pense que") == " et ensuite")
    }

    @Test func acronymsAndTheEnglishIKeepTheirCapitals() {
        #expect(SmartSpacing.adjust("IA générative", after: "sur l'") == "IA générative")
        #expect(SmartSpacing.adjust("I think so", after: "well,") == " I think so")
    }

    @Test func afterALineBreakTheFirstLetterIsCapitalised() {
        #expect(SmartSpacing.adjust("merci", after: "Bonjour,\n") == "Merci")
    }

    @Test func germanNounsKeepTheirCapital() {
        #expect(SmartSpacing.adjust("Hunger habe ich", after: "Jetzt", localeID: "de-DE")
            == " Hunger habe ich")
        #expect(SmartSpacing.adjust("Comment ça va", after: "Salut ", localeID: "fr-FR")
            == "comment ça va")
    }

    @Test func aLeadingLineBreakGetsNoSpace() {
        #expect(SmartSpacing.adjust("\n\nBonjour", after: "Fin.") == "\n\nBonjour")
    }

    @Test func noSpaceAfterAnOpeningBracketOrBeforePunctuation() {
        #expect(SmartSpacing.adjust("voir plus bas", after: "(") == "voir plus bas")
        #expect(SmartSpacing.adjust(", et toi", after: "Salut") == ", et toi")
    }
}

struct VoiceCommandsTests {
    @Test func frenchLineBreakCommands() {
        #expect(VoiceCommands.apply(to: "Bonjour. À la ligne. Merci.", localeID: "fr-FR")
            == "Bonjour.\nMerci.")
        #expect(VoiceCommands.apply(to: "Premier point, nouveau paragraphe, deuxième point.", localeID: "fr-FR")
            == "Premier point\n\nDeuxième point.")
        #expect(VoiceCommands.apply(to: "Liste, point à la ligne, suite", localeID: "fr")
            == "Liste.\nSuite")
    }

    @Test func aColonBeforeACommandIsKept() {
        #expect(VoiceCommands.apply(to: "Voici la liste : nouvelle ligne, pommes", localeID: "fr-FR")
            == "Voici la liste :\nPommes")
    }

    @Test func fullStopCommandNeverDoublesTheFullStop() {
        #expect(VoiceCommands.apply(to: "Fin. Point à la ligne. Suite", localeID: "fr-FR") == "Fin.\nSuite")
        #expect(VoiceCommands.apply(to: "Point à la ligne. Suite", localeID: "fr-FR") == "\nSuite")
    }

    @Test func commandWordsInsideASentenceAreLeftAlone() {
        let sentence = "Je suis allé à la ligne de départ, une nouvelle ligne de métro."
        #expect(VoiceCommands.apply(to: sentence, localeID: "fr-FR") == sentence)
    }

    @Test func englishCommands() {
        #expect(VoiceCommands.apply(to: "Hello. New line. World", localeID: "en-US") == "Hello.\nWorld")
    }

    @Test func commandsFollowTheDictationLanguage() {
        #expect(VoiceCommands.apply(to: "Hello. À la ligne. World", localeID: "en-US")
            == "Hello. À la ligne. World")
        #expect(VoiceCommands.apply(to: "Hello. New line. Salut", localeID: "auto") == "Hello.\nSalut")
    }
}
