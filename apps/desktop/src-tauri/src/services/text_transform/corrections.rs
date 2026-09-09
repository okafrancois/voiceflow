//! Resolve only explicit weekday/numeric corrections; broader revisions need
//! semantic interpretation by the model. Preserve original byte ranges.
use unicode_segmentation::UnicodeSegmentation;

struct Correction<'a> {
    start: usize,
    end: usize,
    from: &'a str,
    to: &'a str,
}

fn value_kind(word: &str) -> Option<bool> {
    if word.chars().all(|c| c.is_numeric()) {
        return Some(true);
    }
    matches!(
        word.to_lowercase().as_str(),
        "lundi"
            | "mardi"
            | "mercredi"
            | "jeudi"
            | "vendredi"
            | "samedi"
            | "dimanche"
            | "monday"
            | "tuesday"
            | "wednesday"
            | "thursday"
            | "friday"
            | "saturday"
            | "sunday"
    )
    .then_some(false)
}

fn corrections(text: &str) -> Vec<Correction<'_>> {
    let words: Vec<_> = text.unicode_word_indices().collect();
    words
        .windows(4)
        .filter_map(|words| {
            let marker = (words[1].1.eq_ignore_ascii_case("non")
                && words[2].1.eq_ignore_ascii_case("pardon"))
                || (words[1].1.eq_ignore_ascii_case("no")
                    && words[2].1.eq_ignore_ascii_case("sorry"));
            let from = words[0].1;
            let to = words[3].1;
            let separated = words.windows(2).all(|pair| {
                text[pair[0].0 + pair[0].1.len()..pair[1].0]
                    .chars()
                    .all(|c| c.is_whitespace() || c == ',')
            });
            let starts_value = text[..words[0].0]
                .chars()
                .next_back()
                .is_none_or(|c| c.is_whitespace());
            (marker
                && separated
                && starts_value
                && value_kind(from).is_some()
                && value_kind(from) == value_kind(to)
                && !from.eq_ignore_ascii_case(to))
            .then_some(Correction {
                start: words[0].0,
                end: words[3].0,
                from,
                to,
            })
        })
        .collect()
}

pub(super) fn resolve(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    for correction in corrections(text) {
        result.push_str(&text[cursor..correction.start]);
        cursor = correction.end;
    }
    result.push_str(&text[cursor..]);
    result
}

pub(super) fn lost(input: &str, output: &str) -> bool {
    let tokens = |text: &str| {
        text.unicode_words()
            .map(str::to_lowercase)
            .collect::<Vec<_>>()
    };
    let resolved = tokens(&resolve(input));
    let result = tokens(output);
    corrections(input).iter().any(|correction| {
        let from = correction.from.to_lowercase();
        let to = correction.to.to_lowercase();
        (resolved.contains(&to) && !result.contains(&to))
            || result.iter().filter(|word| *word == &from).count()
                > resolved.iter().filter(|word| *word == &from).count()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_ambiguous_and_literal_syntax() {
        for text in [
            "Je pense non pardon je sais.",
            "Mardi. Non pardon mercredi.",
            "Le fichier mardi/non/pardon/mercredi est présent.",
            "À 14:30 non pardon 15 heures.",
            "mardi non pardon 14 heures",
        ] {
            assert_eq!(resolve(text), text);
        }
    }

    #[test]
    fn handles_chains_accents_and_punctuation_without_losing_other_values() {
        let input =
            "Prévu mardi, non, pardon, mercredi non pardon jeudi à 14 heures. Mardi reste libre.";
        let expected = "Prévu jeudi à 14 heures. Mardi reste libre.";
        assert_eq!(resolve(input), expected);
        assert!(!lost(input, expected));
        assert!(lost(
            input,
            "Prévu mercredi à 14 heures. Mardi reste libre."
        ));
        assert_eq!(
            resolve("Meet Monday no sorry Tuesday at 14."),
            "Meet Tuesday at 14."
        );
        assert_eq!(resolve("250 non pardon 300 euros"), "300 euros");
    }
}
