import Foundation
import NaturalLanguage

/// Espace et majuscule selon ce qui précède le curseur.
///
/// Les moteurs rendent chaque dictée comme une phrase isolée : majuscule en
/// tête, aucune espace devant. Deux dictées d'affilée donnaient donc
/// « …la première.La seconde », et une dictée en milieu de phrase une
/// majuscule parasite. Quand le champ dit ce qui précède l'insertion, on
/// raccorde ; quand il ne le dit pas, on ne touche à rien.
enum SmartSpacing {
    private static let sentenceEnds: Set<Character> = [".", "!", "?", "…"]
    /// Après eux, on colle le texte sans espace.
    private static let openers: Set<Character> = ["(", "[", "{", "\"", "'", "’", "«", "“", "‘", "/", "-", "@", "#"]
    /// En tête de dictée, ils se collent au mot précédent.
    private static let leadingPunctuation: Set<Character> = [",", ".", ";", ":", "!", "?", ")", "]", "}", "…"]

    /// Langues où un mot courant ne prend pas de majuscule en milieu de
    /// phrase. L'allemand, qui en met à tous les noms, n'en fait pas partie.
    private static let lowercaseLanguages: Set<String> = [
        "fr", "en", "es", "it", "pt", "ca", "nl", "sv", "da", "nb", "no", "pl", "ro",
    ]

    /// `localeID` : langue de la dictée ; en détection automatique, celle du
    /// texte lui-même.
    static func adjust(_ text: String, after context: String?, localeID: String? = nil) -> String {
        let body = String(text.drop(while: { $0 == " " || $0 == "\t" }))
        guard let context, let last = context.last, let first = body.first else { return text }

        if last.isNewline {
            return capitalizingFirst(body)
        }
        // La dictée commence par un saut de ligne : rien à raccorder.
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
        // Langue inconnue : dans le doute, ne rien toucher.
        guard let code else { return localeID == nil }
        return lowercaseLanguages.contains(code)
    }

    private static func capitalizingFirst(_ text: String) -> String {
        guard let first = text.first else { return text }
        return first.uppercased() + text.dropFirst()
    }

    /// Minuscule en milieu de phrase, sauf pour ce qui s'écrit toujours avec
    /// une majuscule : sigles, « I » anglais, noms propres, graphies mixtes
    /// (iPhone, McDonald).
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

/// Commandes de mise en forme dites pendant la dictée : « à la ligne »,
/// « nouveau paragraphe », « new line »…
///
/// Une commande n'est reconnue que seule entre deux ponctuations (ou en
/// début/fin de dictée) : « la ligne de départ » ou « une nouvelle ligne de
/// métro » restent du texte. Une commande dite sans pause peut donc être
/// manquée ; c'est le prix de ne jamais casser une phrase ordinaire.
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

    /// Avant la commande : début du texte ou ponctuation forte (conservée),
    /// ou virgule / point-virgule (absorbés). Après : ponctuation ou fin.
    private static func replace(_ command: Command, in text: String) -> String {
        let phrase = NSRegularExpression.escapedPattern(for: command.phrase)
            .replacingOccurrences(of: "à", with: "[àa]")
        let tail = #"[ \t]*(?:[,.;:!?…]|$)[ \t]*"#
        // Après une fin de phrase ou en tête, « point à la ligne » ne remet
        // pas un second point.
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
