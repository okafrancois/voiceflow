mod corrections;

use crate::polish_engine::{PolishRequest, PolishResult, UnifiedPolishManager};
use crate::state::app_state::AppState;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformIntent {
    Cleanup,
    Concise,
    Translate,
    Reply,
    Rewrite,
}

impl TransformIntent {
    pub fn for_template(template_id: Option<&str>) -> Self {
        if template_id == Some("concise") {
            Self::Concise
        } else {
            Self::Cleanup
        }
    }
}

pub struct TransformOutcome {
    pub result: PolishResult,
    pub rejection_reason: Option<&'static str>,
}

pub fn accept_output(
    input: &str,
    output: &str,
    intent: TransformIntent,
) -> (String, Option<&'static str>) {
    let reason = rejection_reason(input, output, intent);
    let text = if reason.is_some() {
        input
    } else {
        output.trim()
    };
    (text.to_string(), reason)
}

pub(crate) fn prepare_transcript(text: &str, intent: TransformIntent) -> String {
    if matches!(intent, TransformIntent::Cleanup | TransformIntent::Concise) {
        corrections::resolve(text)
    } else {
        text.to_string()
    }
}

/// All product entry points use this service; engines only implement provider protocols.
pub async fn transform_text(
    state: &AppState,
    mut request: PolishRequest,
    intent: TransformIntent,
) -> Result<TransformOutcome, String> {
    let (cloud_enabled, provider, cloud_config, model_id) = {
        let settings = state.settings.lock();
        (
            settings.cloud_polish_enabled,
            settings.active_cloud_polish_provider.clone(),
            settings
                .cloud_polish_configs
                .get(&settings.active_cloud_polish_provider)
                .cloned(),
            settings.polish_model.clone(),
        )
    };
    let original = request.text.clone();
    request.text = prepare_transcript(&original, intent);
    let timeout = if cloud_enabled {
        Duration::from_secs(60)
    } else if crate::polish_engine::qwen::uses_reasoning(&model_id) {
        LOCAL_POLISH_MAX_TIMEOUT
    } else {
        local_polish_timeout(&original)
    };
    request = request.with_timeout(timeout);
    let mut result = if cloud_enabled {
        let config =
            cloud_config.ok_or_else(|| "Cloud polish configuration is missing".to_string())?;
        if config.api_key.trim().is_empty() || config.model.trim().is_empty() {
            return Err("Cloud polish credentials and model are required".to_string());
        }
        tokio::time::timeout(
            timeout,
            state.polish_manager.polish_cloud(
                request,
                &provider,
                &config.api_key,
                &config.base_url,
                &config.model,
                config.enable_thinking,
            ),
        )
        .await
        .map_err(|_| "Cloud polish timed out".to_string())??
    } else {
        let engine = UnifiedPolishManager::get_engine_by_model_id(&model_id)
            .ok_or_else(|| "Select a local polish model first".to_string())?;
        if !state.polish_manager.is_model_downloaded(engine, &model_id) {
            return Err("Polish model is not downloaded".to_string());
        }
        let filename = state
            .polish_manager
            .get_model_filename(engine, &model_id)
            .ok_or_else(|| "Polish model file is missing".to_string())?;
        tokio::time::timeout(
            timeout,
            state
                .polish_manager
                .polish(engine, request.with_model(filename)),
        )
        .await
        .map_err(|_| LOCAL_POLISH_TIMEOUT_REASON.to_string())??
    };
    let (text, rejection_reason) = accept_output(&original, &result.text, intent);
    result.text = text;
    if let Some(reason) = rejection_reason {
        tracing::warn!(reason, "transform_output_rejected-using_original");
    }
    Ok(TransformOutcome {
        result,
        rejection_reason,
    })
}

pub(crate) const LOCAL_POLISH_BASE_TIMEOUT: Duration = Duration::from_secs(20);
pub(crate) const LOCAL_POLISH_MAX_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const LOCAL_POLISH_BASE_TIMEOUT_CHARS: usize = 500;
pub(crate) const LOCAL_POLISH_TIMEOUT_STEP_CHARS: usize = 800;
pub(crate) const LOCAL_POLISH_TIMEOUT_STEP: Duration = Duration::from_secs(10);
pub(crate) const LOCAL_POLISH_TIMEOUT_REASON: &str = "local polish timed out";

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

pub(crate) fn local_polish_timeout(text: &str) -> Duration {
    let chars = text.chars().count();
    let extra_chars = chars.saturating_sub(LOCAL_POLISH_BASE_TIMEOUT_CHARS);
    let extra_steps = extra_chars.div_ceil(LOCAL_POLISH_TIMEOUT_STEP_CHARS);
    let timeout = LOCAL_POLISH_BASE_TIMEOUT
        + Duration::from_secs(LOCAL_POLISH_TIMEOUT_STEP.as_secs() * extra_steps as u64);

    timeout.min(LOCAL_POLISH_MAX_TIMEOUT)
}

fn has_question_mark(text: &str) -> bool {
    text.contains('?') || text.contains('？')
}

fn is_question_like_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    has_question_mark(text)
        || contains_any(
            &lower,
            &[
                "吗",
                "是不是",
                "是否",
                "哪些",
                "哪个",
                "哪里",
                "哪儿",
                "为什么",
                "怎么",
                "如何",
                "有没有",
                "能不能",
                "可不可以",
                "what",
                "why",
                "how",
                "should",
                "could",
                "would",
            ],
        )
}

fn is_answer_like_text(text: &str) -> bool {
    let lower = text
        .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ',' | '，' | '.' | '。'))
        .to_lowercase();

    lower.starts_with("我觉得")
        || lower.starts_with("我认为")
        || lower.starts_with("是的")
        || lower.starts_with("不是")
        || lower.starts_with("可以")
        || lower.starts_with("不可以")
        || lower.starts_with("不能")
        || lower.starts_with("还不")
        || lower.starts_with("还没")
        || lower.starts_with("需要")
        || lower.starts_with("不需要")
        || contains_any(
            &lower,
            &[
                "不够完整",
                "还没到",
                "还不是",
                "不是所有",
                "not ready",
                "is ready",
                "is not ready",
                "i think",
                "i believe",
                "je ne peux pas",
                "je peux vous aider",
                "je peux t'aider",
                "je suis désolé",
                "i cannot",
                "i can help",
            ],
        )
}

pub(crate) fn should_reject_question_answer_polish(input: &str, output: &str) -> bool {
    (has_question_mark(input) && !has_question_mark(output))
        || (is_question_like_text(input)
            && is_answer_like_text(output)
            && !is_answer_like_text(input))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectedTextLanguage {
    French,
    English,
}

fn language_score(text: &str, words: &[&str]) -> usize {
    text.split(|character: char| !character.is_alphabetic())
        .filter(|token| !token.is_empty())
        .map(str::to_lowercase)
        .filter(|token| words.contains(&token.as_str()))
        .count()
}

fn detect_supported_language(text: &str) -> Option<DetectedTextLanguage> {
    const FRENCH_WORDS: &[&str] = &[
        "alors", "après", "avec", "avant", "bien", "ça", "ce", "cette", "dans", "de", "des", "du",
        "elle", "elles", "en", "est", "et", "faire", "il", "ils", "je", "la", "le", "les", "mais",
        "mon", "nous", "pas", "plus", "pour", "quand", "que", "qui", "quoi", "sont", "très", "tu",
        "un", "une", "vous", "vérifie",
    ];
    const ENGLISH_WORDS: &[&str] = &[
        "add", "and", "are", "as", "be", "can", "check", "create", "do", "does", "error", "fix",
        "for", "from", "has", "have", "in", "input", "is", "not", "of", "on", "output", "please",
        "should", "task", "that", "the", "then", "they", "this", "to", "we", "will", "with",
        "would", "you", "your",
    ];

    let french = language_score(text, FRENCH_WORDS);
    let english = language_score(text, ENGLISH_WORDS);
    if french >= 3 && french >= english.saturating_add(2) {
        Some(DetectedTextLanguage::French)
    } else if english >= 3 && english >= french.saturating_add(2) {
        Some(DetectedTextLanguage::English)
    } else {
        None
    }
}

fn meaningful_char_count(text: &str) -> usize {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .count()
}

/// Discount only adjacent, identical complete sentences, never scattered words or
/// short repeated steps. This changes the length baseline, not the transcript.
fn repetition_adjusted_char_count(text: &str) -> usize {
    let mut previous = "";
    text.split_inclusive(['.', '!', '?', '。', '！', '？'])
        .map(|part| {
            let sentence = part.trim();
            let complete = sentence.ends_with(['.', '!', '?', '。', '！', '？']);
            let duplicate =
                complete && sentence.split_whitespace().count() >= 8 && sentence == previous;
            previous = sentence;
            if duplicate {
                0
            } else {
                meaningful_char_count(part)
            }
        })
        .sum()
}

/// Spoken ordinal words carry layout rather than facts when represented by
/// actual list items. Discount only repeated markers, never ordinary prose.
fn enumeration_marker_discount(input: &str, output: &str) -> usize {
    let list_items = output
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("- ")
                || line.starts_with("• ")
                || line
                    .split_once(". ")
                    .is_some_and(|(prefix, _)| prefix.parse::<usize>().is_ok())
        })
        .count();
    if list_items < 2 {
        return 0;
    }
    let markers: std::collections::HashSet<_> = input
        .split(|c: char| !c.is_alphabetic())
        .map(str::to_lowercase)
        .filter(|word| {
            matches!(
                word.as_str(),
                "premièrement"
                    | "deuxièmement"
                    | "troisièmement"
                    | "quatrièmement"
                    | "cinquièmement"
                    | "firstly"
                    | "secondly"
                    | "thirdly"
                    | "fourthly"
                    | "fifthly"
            )
        })
        .collect();
    if markers.len() < 2 || list_items < markers.len() {
        return 0;
    }
    markers.iter().map(|word| word.chars().count()).sum()
}

pub(crate) fn rejection_reason(
    input: &str,
    output: &str,
    intent: TransformIntent,
) -> Option<&'static str> {
    if output.trim().is_empty() {
        return Some("model returned empty output");
    }
    if intent == TransformIntent::Reply {
        return None;
    }
    if should_reject_question_answer_polish(input, output) {
        return Some("polish answered dictated question");
    }

    if matches!(intent, TransformIntent::Cleanup | TransformIntent::Concise)
        && corrections::lost(input, output)
    {
        return Some("polish lost an explicit correction");
    }

    let question_count = |text: &str| text.chars().filter(|c| matches!(c, '?' | '？')).count();
    if question_count(input) > 0 && question_count(output) > question_count(input) {
        return Some("polish added questions");
    }

    if intent != TransformIntent::Translate {
        let input_language = detect_supported_language(input);
        let output_language = detect_supported_language(output);
        if input_language.is_some()
            && output_language.is_some()
            && input_language != output_language
        {
            return Some("polish changed transcript language");
        }
    }

    let input_chars = meaningful_char_count(input);
    if input_chars >= 120
        && !matches!(
            intent,
            TransformIntent::Translate | TransformIntent::Rewrite
        )
    {
        let output_chars = meaningful_char_count(output);
        let minimum_ratio = if intent == TransformIntent::Concise {
            0.30
        } else {
            0.55
        };
        if (output_chars as f64)
            < (repetition_adjusted_char_count(input)
                .saturating_sub(enumeration_marker_discount(input, output)) as f64
                * minimum_ratio)
        {
            return Some("polish removed too much transcript content");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_cleanup_of_consecutive_duplicate_sentences() {
        let sentence = "Je veux vérifier le suivi des événements et les règles de sécurité.";
        let input = format!("{sentence} {sentence} {sentence}");
        let (output, reason) = accept_output(&input, sentence, TransformIntent::Cleanup);
        assert_eq!(reason, None);
        assert_eq!(output, sentence);
    }

    #[test]
    fn repetition_discount_does_not_hide_distinct_requirements() {
        let repeated = "Je veux vérifier le suivi des événements et les règles de sécurité.";
        let distinct = "Il faut conserver les identifiants des utilisateurs sans les transmettre au serveur. Il faut également tester la suppression des fichiers temporaires et documenter toutes les erreurs de connexion.";
        let input = format!("{repeated} {repeated} {repeated} {distinct}");
        assert_eq!(
            rejection_reason(&input, repeated, TransformIntent::Cleanup),
            Some("polish removed too much transcript content")
        );
    }

    #[test]
    fn accepts_list_without_spoken_enumeration_markers() {
        let input = "Je veux vérifier trois points. Premièrement, le tracking des événements. Deuxièmement, la sécurité des données. Troisièmement, les nouvelles fonctionnalités de la version 1.2.2.";
        let output = "- tracking des événements\n- sécurité des données\n- nouvelles fonctionnalités de la version 1.2.2";
        assert_eq!(
            rejection_reason(input, output, TransformIntent::Cleanup),
            None
        );
    }

    #[test]
    fn repetition_baseline_preserves_changed_facts_and_short_steps() {
        for input in [
            "Tourne à gauche. Tourne à gauche. Tourne à gauche.",
            "Il faut conserver les données pendant exactement 30 jours. Il faut conserver les données pendant exactement 60 jours.",
            "Il faut conserver les données et transmettre les identifiants au serveur. Il faut conserver les données et ne pas transmettre les identifiants au serveur.",
            "Je veux vérifier le suivi des événements et les règles de sécurité",
        ] {
            assert_eq!(repetition_adjusted_char_count(input), meaningful_char_count(input));
        }
    }

    #[test]
    fn rejects_french_assistant_answers_and_invented_question_tasks() {
        let input = "Est-ce que tu peux vérifier si la sécurité de cette application est suffisante pour publier la version 1.2.2 ?";
        for output in [
            "Je ne peux pas vérifier directement la sécurité de l'application. Cependant, je peux vous aider à analyser les aspects de sécurité.",
            "Je peux vous aider à vérifier la sécurité. Quels aspects souhaitez-vous examiner ?",
            "- Vérifier la sécurité de l'application version 1.2.2\n- Documenter les résultats et confirmer les protocoles.",
        ] {
            assert_eq!(rejection_reason(input, output, TransformIntent::Cleanup),
                Some("polish answered dictated question"));
        }
    }

    #[test]
    fn resolves_unambiguous_single_value_corrections_before_inference() {
        assert_eq!(
            prepare_transcript(
                "On se retrouve mardi non pardon mercredi à 14 heures.",
                TransformIntent::Cleanup
            ),
            "On se retrouve mercredi à 14 heures."
        );
        assert_eq!(
            prepare_transcript(
                "Le budget est de 250 non pardon 300 euros.",
                TransformIntent::Concise
            ),
            "Le budget est de 300 euros."
        );
        let ambiguous = "Je pense non pardon je sais que le serveur est prêt.";
        assert_eq!(
            prepare_transcript(ambiguous, TransformIntent::Cleanup),
            ambiguous
        );
        let reply = "On se retrouve mardi non pardon mercredi.";
        assert_eq!(prepare_transcript(reply, TransformIntent::Reply), reply);
    }

    #[test]
    fn rejects_lost_explicit_self_correction() {
        let input = "Salut Camille, on se retrouve mardi non pardon mercredi à 14 heures au café République, ça te va ?";
        let wrong =
            "Salut Camille, on se retrouve mardi à 14 heures au café République, ça te va ?";
        assert_eq!(
            rejection_reason(input, wrong, TransformIntent::Cleanup),
            Some("polish lost an explicit correction")
        );
        let correct =
            "Salut Camille, on se retrouve mercredi à 14 heures au café République, ça te va ?";
        assert_eq!(
            rejection_reason(input, correct, TransformIntent::Cleanup),
            None
        );
    }

    #[test]
    fn rejects_invented_question_expansion() {
        let input = "Peux-tu vérifier la sécurité de cette application avant la publication de la version 1.2.2 ?";
        let output = "Peux-tu vérifier les vulnérabilités connues de cette application ? Peux-tu confirmer les protocoles de sécurité ? Peux-tu fournir un rapport des risques ?";
        assert_eq!(
            rejection_reason(input, output, TransformIntent::Cleanup),
            Some("polish added questions")
        );
    }

    #[test]
    fn list_discount_requires_a_complete_list_and_keeps_distinct_content() {
        let input = "Premièrement vérifier les événements. Deuxièmement contrôler les accès. Troisièmement conserver les journaux pendant 30 jours et vérifier les permissions de chaque utilisateur.";
        assert_eq!(
            enumeration_marker_discount(input, "Premièrement vérifier les événements."),
            0
        );
        assert_eq!(
            enumeration_marker_discount(input, "- Événements\n- Accès"),
            0
        );
        assert_eq!(
            rejection_reason(
                input,
                "- Événements\n- Accès\n- Journaux",
                TransformIntent::Cleanup
            ),
            Some("polish removed too much transcript content")
        );
    }

    #[test]
    fn accepts_dictated_answers_and_explicit_reply_intent() {
        let text = "Je ne peux pas vérifier cette application. Peux-tu demander à Camille ?";
        assert_eq!(rejection_reason(text, text, TransformIntent::Cleanup), None);
        assert_eq!(
            rejection_reason(
                "Peux-tu vérifier cette application ?",
                "Je peux vous aider.",
                TransformIntent::Reply
            ),
            None
        );
    }

    #[test]
    fn unsafe_cleanup_returns_original_text() {
        let source = "Alors je veux que tu vérifies les données et les règles de sécurité dans cette application.";
        let (text, reason) = accept_output(
            source,
            "Please fix the error and check the output.",
            TransformIntent::Cleanup,
        );
        assert_eq!(text, source);
        assert_eq!(reason, Some("polish changed transcript language"));
    }

    #[test]
    fn explicit_reply_can_answer_but_cleanup_cannot() {
        let source = "Is the project ready?";
        let answer = "I think the project is ready.";
        assert_eq!(
            accept_output(source, answer, TransformIntent::Cleanup).0,
            source
        );
        assert_eq!(
            accept_output(source, answer, TransformIntent::Reply).0,
            answer
        );
    }

    #[test]
    fn explicit_translation_accepts_language_change() {
        let source = "Please check the application and the output.";
        let translated = "Vérifie les résultats et les données de cette application.";
        assert_eq!(
            accept_output(source, translated, TransformIntent::Translate),
            (translated.to_string(), None)
        );
    }

    #[test]
    fn empty_output_never_replaces_source_for_any_intent() {
        for intent in [
            TransformIntent::Cleanup,
            TransformIntent::Concise,
            TransformIntent::Translate,
            TransformIntent::Reply,
            TransformIntent::Rewrite,
        ] {
            assert_eq!(accept_output("Keep this", " ", intent).0, "Keep this");
        }
    }
}
