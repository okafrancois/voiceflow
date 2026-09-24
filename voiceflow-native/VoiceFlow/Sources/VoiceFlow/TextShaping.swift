import Foundation
import NaturalLanguage

/// Spacing and capitalization based on what precedes the cursor.
///
/// Engines render each dictation as an isolated sentence: capitalized at
/// the start, no leading space. Two dictations in a row therefore produced
/// « …la première.La seconde », and a dictation mid-sentence produced a
/// stray capital letter. When the field reports what precedes the
/// insertion, we stitch it together; when it doesn't, we leave it alone.
enum SmartSpacing {
    private static let sentenceEnds: Set<Character> = [".", "!", "?", "…"]
    /// After these, the text is glued on without a space.
    private static let openers: Set<Character> = ["(", "[", "{", "\"", "'", "’", "«", "“", "‘", "/", "-", "@", "#"]
    /// At the start of a dictation, these glue onto the previous word.
    private static let leadingPunctuation: Set<Character> = [",", ".", ";", ":", "!", "?", ")", "]", "}", "…"]

    /// Languages where a common word doesn't take a capital letter
    /// mid-sentence. German, which capitalizes all nouns, is not among them.
    private static let lowercaseLanguages: Set<String> = [
        "fr", "en", "es", "it", "pt", "ca", "nl", "sv", "da", "nb", "no", "pl", "ro",
    ]

    /// `localeID`: language of the dictation; in automatic detection, that
    /// of the text itself.
    static func adjust(_ text: String, after context: String?, localeID: String? = nil) -> String {
        let body = String(text.drop(while: { $0 == " " || $0 == "\t" }))
        guard let context, let last = context.last, let first = body.first else { return text }

        if last.isNewline {
            return capitalizingFirst(body)
        }
        // The dictation starts with a line break: nothing to stitch together.
        if first.isNewline { return body }

        let needsSpace = !last.isWhitespace
            && !openers.contains(last)
            && !leadingPunctuation.contains(first)

        let trimmedContext = context.trimmingCharacters(in: .whitespaces)
        let endsSentence = trimmedContext.last.map { sentenceEnds.contains($0) } ?? true
        let shaped: String
        if endsSentence {
            shaped = capitalizingFirst(body)
        } else if allowsLowercasing(body, localeID: localeID) {
            shaped = lowercasingFirstIfCommonWord(body)
        } else {
            shaped = body
        }
        return needsSpace ? " " + shaped : shaped
    }

    private static func allowsLowercasing(_ text: String, localeID: String?) -> Bool {
        let code: String?
        if let localeID, localeID != AppState.autoLocaleID {
            code = Locale(identifier: localeID).language.languageCode?.identifier
        } else {
            code = NLLanguageRecognizer.dominantLanguage(for: text)?.rawValue
        }
        // Unknown language: when in doubt, don't touch anything.
        guard let code else { return localeID == nil }
        return lowercaseLanguages.contains(code)
    }

    private static func capitalizingFirst(_ text: String) -> String {
        guard let first = text.first else { return text }
        return first.uppercased() + text.dropFirst()
    }

    /// Lowercase mid-sentence, except for what is always written with a
    /// capital letter: acronyms, English "I", proper nouns, mixed-case
    /// spellings (iPhone, McDonald).
    private static func lowercasingFirstIfCommonWord(_ text: String) -> String {
        let firstWord = text.prefix { $0.isLetter }
        guard let initial = firstWord.first, initial.isUppercase else { return text }
        guard firstWord.count > 1, firstWord.dropFirst().allSatisfy(\.isLowercase) else { return text }
        guard !isProperName(text, word: String(firstWord)) else { return text }
        return initial.lowercased() + text.dropFirst()
    }

    private static func isProperName(_ text: String, word: String) -> Bool {
        let tagger = NLTagger(tagSchemes: [.nameType])
        tagger.string = text
        let range = text.startIndex..<text.index(text.startIndex, offsetBy: word.count)
        let (tag, _) = tagger.tag(at: range.lowerBound, unit: .word, scheme: .nameType)
        return tag == .personalName || tag == .placeName || tag == .organizationName
    }
}

/// Formatting commands spoken during dictation: « à la ligne »,
/// « nouveau paragraphe », « new line »…
///
/// A command is only recognized on its own between two punctuation marks
/// (or at the start/end of dictation): « la ligne de départ » or « une
/// nouvelle ligne de métro » remain plain text. A command spoken without a
/// pause can therefore be missed; that's the price of never breaking an
/// ordinary sentence.
enum VoiceCommands {
    private struct Command {
        let phrase: String
        let output: String
    }

    private static let french: [Command] = [
        Command(phrase: "point à la ligne", output: ".\n"),
        Command(phrase: "retour à la ligne", output: "\n"),
        Command(phrase: "nouveau paragraphe", output: "\n\n"),
        Command(phrase: "nouvelle ligne", output: "\n"),
        Command(phrase: "à la ligne", output: "\n"),
    ]

    private static let english: [Command] = [
        Command(phrase: "new paragraph", output: "\n\n"),
        Command(phrase: "new line", output: "\n"),
    ]

    static func apply(to text: String, localeID: String) -> String {
        let commands = commands(for: localeID)
        guard !commands.isEmpty else { return text }
        var result = text
        for command in commands {
            result = replace(command, in: result)
        }
        return capitalizingAfterLineBreaks(result)
    }

    private static func commands(for localeID: String) -> [Command] {
        if localeID == AppState.autoLocaleID { return french + english }
        switch Locale(identifier: localeID).language.languageCode?.identifier {
        case "fr": return french
        case "en": return english
        default: return []
        }
    }

    /// Before the command: start of text or strong punctuation (kept),
    /// or comma / semicolon (absorbed). After: punctuation or end.
    private static func replace(_ command: Command, in text: String) -> String {
        let phrase = NSRegularExpression.escapedPattern(for: command.phrase)
            .replacingOccurrences(of: "à", with: "[àa]")
        let tail = #"[ \t]*(?:[,.;:!?…]|$)[ \t]*"#
        // After a sentence end or at the start, « point à la ligne » does
        // not add a second period.
        let afterBoundary = #"(?:^|(?<=[.!?…:]))[ \t]*"# + phrase + tail
        let afterComma = #"[ \t]*[,;][ \t]*"# + phrase + tail
        let boundaryOutput = command.output.hasPrefix(".")
            ? String(command.output.dropFirst()) : command.output
        var result = replacing(afterBoundary, in: text, with: boundaryOutput)
        result = replacing(afterComma, in: result, with: command.output)
        return result
    }

    private static func replacing(_ pattern: String, in text: String, with output: String) -> String {
        guard let regex = try? NSRegularExpression(
            pattern: pattern, options: [.caseInsensitive, .anchorsMatchLines])
        else { return text }
        let range = NSRange(text.startIndex..., in: text)
        let template = NSRegularExpression.escapedTemplate(for: output)
        return regex.stringByReplacingMatches(in: text, range: range, withTemplate: template)
    }

    private static func capitalizingAfterLineBreaks(_ text: String) -> String {
        var result = ""
        var capitalizeNext = false
        for character in text {
            if character.isNewline {
                capitalizeNext = true
                result.append(character)
            } else if capitalizeNext, character.isLetter {
                result += character.uppercased()
                capitalizeNext = false
            } else {
                if !character.isWhitespace { capitalizeNext = false }
                result.append(character)
            }
        }
        return result
    }
}
