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
    let timeout = if cloud_enabled {
        Duration::from_secs(60)
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
            ],
        )
}

pub(crate) fn should_reject_question_answer_polish(input: &str, output: &str) -> bool {
    is_question_like_text(input) && !has_question_mark(output) && is_answer_like_text(output)
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
        if (output_chars as f64) < (input_chars as f64 * minimum_ratio) {
            return Some("polish removed too much transcript content");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
