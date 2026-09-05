use super::postprocess::strip_trailing_sentence_period;
use super::shared::ProcessingEventTarget;
use crate::polish_engine::{get_template_by_id, PolishRequest, DEFAULT_POLISH_PROMPT};
use crate::services::text_transform::{
    transform_text, TransformIntent, LOCAL_POLISH_TIMEOUT_REASON,
};
use crate::state::app_state::AppState;
use std::time::Instant;
use tracing::{info, instrument, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PolishProcessingResult {
    pub text: String,
    pub polish_ms: u64,
    pub polish_wall_ms: u64,
    pub polish_queue_ms: u64,
    pub model_load_ms: Option<u64>,
    pub context_create_ms: Option<u64>,
    pub prefill_ms: Option<u64>,
    pub inference_ms: Option<u64>,
    pub time_to_first_token_ms: Option<u64>,
    pub generation_ms: Option<u64>,
    pub fallback_reason: Option<&'static str>,
}

impl PolishProcessingResult {
    pub(super) fn skipped(text: String, fallback_reason: &'static str) -> Self {
        let (text, _) = strip_trailing_sentence_period(&text);
        Self {
            text,
            polish_ms: 0,
            polish_wall_ms: 0,
            polish_queue_ms: 0,
            model_load_ms: None,
            context_create_ms: None,
            prefill_ms: None,
            inference_ms: None,
            time_to_first_token_ms: None,
            generation_ms: None,
            fallback_reason: Some(fallback_reason),
        }
    }

    fn from_polish_result(
        result: crate::polish_engine::PolishResult,
        polish_wall_ms: u64,
        polish_queue_ms: u64,
    ) -> Self {
        let (text, _) = strip_trailing_sentence_period(&result.text);
        Self {
            text,
            polish_ms: result.total_ms,
            polish_wall_ms,
            polish_queue_ms,
            model_load_ms: result.model_load_ms,
            context_create_ms: result.context_create_ms,
            prefill_ms: result.prefill_ms,
            inference_ms: result.inference_ms,
            time_to_first_token_ms: result.time_to_first_token_ms,
            generation_ms: result.generation_ms,
            fallback_reason: None,
        }
    }

    fn using_original(
        text: String,
        polish_wall_ms: u64,
        polish_queue_ms: u64,
        fallback_reason: &'static str,
    ) -> Self {
        let (text, _) = strip_trailing_sentence_period(&text);
        Self {
            text,
            polish_ms: 0,
            polish_wall_ms,
            polish_queue_ms,
            model_load_ms: None,
            context_create_ms: None,
            prefill_ms: None,
            inference_ms: None,
            time_to_first_token_ms: None,
            generation_ms: None,
            fallback_reason: Some(fallback_reason),
        }
    }
}

fn classify_polish_failure_reason(error: &str) -> &'static str {
    let lower = error.to_lowercase();

    if lower == "select a local polish model first" {
        return "no polish model selected";
    }
    if lower == "cloud polish configuration is missing"
        || lower == "cloud polish credentials and model are required"
    {
        return "cloud polish configuration incomplete";
    }

    if lower.contains("incomplete") {
        "model download looks incomplete"
    } else if is_local_polish_timeout_error(error) {
        LOCAL_POLISH_TIMEOUT_REASON
    } else if lower.contains("not downloaded") || lower.contains("not found") {
        "model file is missing"
    } else if lower.contains("model load")
        || lower.contains("context")
        || lower.contains("null reference")
        || lower.contains("out of memory")
        || lower.contains("memory")
    {
        "model could not be loaded, likely low memory"
    } else if lower.contains("backend init") {
        "polish runtime failed to start"
    } else if lower.contains("tokenize") || lower.contains("tokenization") {
        "input could not be tokenized"
    } else if lower.contains("decode") || lower.contains("inference") {
        "model inference failed"
    } else if lower.contains("task join") || lower.contains("panic") {
        "polish worker crashed"
    } else if lower.contains("local polish server unavailable") {
        "local polish server unavailable"
    } else if lower.contains("401") || lower.contains("unauthorized") || lower.contains("api key") {
        "cloud polish authentication failed"
    } else if lower.contains("429") || lower.contains("rate limit") {
        "cloud polish was rate limited"
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "cloud polish timed out"
    } else if lower.contains("network")
        || lower.contains("connection")
        || lower.contains("dns")
        || lower.contains("resolve")
    {
        "cloud polish network failed"
    } else {
        "unexpected polish error"
    }
}

fn is_local_polish_timeout_error(error: &str) -> bool {
    error.to_lowercase().contains(LOCAL_POLISH_TIMEOUT_REASON)
}

#[instrument(
    skip(event_target, state, accumulated_text, resolved_polish_template_id),
    fields(task_id)
)]
pub(super) async fn maybe_polish_transcription_text(
    event_target: &ProcessingEventTarget<'_>,
    state: &AppState,
    task_id: u64,
    accumulated_text: String,
    resolved_polish_template_id: Option<String>,
) -> PolishProcessingResult {
    maybe_polish_transcription_text_for_profile(
        event_target,
        state,
        task_id,
        accumulated_text,
        resolved_polish_template_id,
        None,
        None,
    )
    .await
}

pub(super) async fn maybe_polish_transcription_text_for_profile(
    event_target: &ProcessingEventTarget<'_>,
    state: &AppState,
    task_id: u64,
    accumulated_text: String,
    template_id: Option<String>,
    profile: Option<&crate::services::product_workflows::WorkflowProfile>,
    vibe_context: Option<&crate::services::platform_quality::CodeContext>,
) -> PolishProcessingResult {
    let instruction = profile.and_then(workflow_profile_instruction);
    if template_id.is_none() && instruction.is_none() {
        return PolishProcessingResult::skipped(accumulated_text, "no polish template");
    }
    let (mut prompt, language) = {
        let settings = state.settings.lock();
        let prompt = template_id
            .as_deref()
            .and_then(get_template_by_id)
            .map(|template| template.system_prompt.to_string())
            .or_else(|| {
                settings
                    .polish_custom_templates
                    .iter()
                    .find(|template| Some(&template.id) == template_id.as_ref())
                    .map(|template| template.system_prompt.clone())
            })
            .unwrap_or_else(|| DEFAULT_POLISH_PROMPT.to_string());
        let language = profile
            .and_then(|profile| profile.language.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| settings.stt_engine_language.clone());
        (prompt, language)
    };
    let translation = profile
        .and_then(|profile| profile.translation_target.as_deref())
        .filter(|value| !value.trim().is_empty());
    let intent = if translation.is_some() {
        TransformIntent::Translate
    } else {
        TransformIntent::for_template(template_id.as_deref())
    };
    if let Some(instruction) = instruction {
        if translation.is_some() {
            prompt = instruction;
        } else {
            prompt.push_str(&format!("\n\n{instruction}"));
        }
    }
    if let Some(context) = vibe_context {
        prompt.push_str("\n\n");
        prompt.push_str(&crate::services::platform_quality::build_code_aware_instruction(context));
    }
    let mut request = PolishRequest::new(
        accumulated_text.clone(),
        prompt,
        translation.unwrap_or(&language),
    );
    if let Some(context) = state
        .session_state
        .lock()
        .as_ref()
        .and_then(|session| session.window_context.as_ref())
        .and_then(|context| context.to_polish_context())
    {
        request = request.with_window_context(context);
    }
    if let Some(callback) = event_target.polish_preview_callback(task_id) {
        request = request.with_preview_callback(callback);
    }
    event_target.emit_polishing(task_id);
    let started = Instant::now();
    match transform_text(state, request, intent).await {
        Ok(outcome) => {
            let wall_ms = started.elapsed().as_millis() as u64;
            if let Some(reason) = outcome.rejection_reason {
                event_target.emit_polish_policy_tooltip(task_id);
                PolishProcessingResult::using_original(accumulated_text, wall_ms, 0, reason)
            } else {
                info!(task_id, polish_wall_ms = wall_ms, "polish_completed");
                PolishProcessingResult::from_polish_result(outcome.result, wall_ms, 0)
            }
        }
        Err(error) => {
            let reason = classify_polish_failure_reason(&error);
            if is_local_polish_timeout_error(&error) {
                event_target.emit_local_polish_timeout_tooltip(task_id);
            } else {
                event_target.emit_polish_error_tooltip(task_id, Some(reason));
            }
            warn!(task_id, error, "polish_failed-using_original");
            PolishProcessingResult::using_original(
                accumulated_text,
                started.elapsed().as_millis() as u64,
                0,
                reason,
            )
        }
    }
}

fn workflow_profile_instruction(
    profile: &crate::services::product_workflows::WorkflowProfile,
) -> Option<String> {
    let mut instructions = Vec::new();
    if let Some(target) = profile
        .translation_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
    {
        instructions.push(format!(
            "Translate the complete output to {target}. Do not keep it in the source language."
        ));
    }
    if profile.code_aware {
        let mut context = crate::services::platform_quality::get_active_code_context()
            .ok()
            .flatten()
            .unwrap_or_default();
        if context.language.is_none() {
            context.language = profile.language.clone();
        }
        instructions
            .push(crate::services::platform_quality::build_code_aware_instruction(&context));
    }
    (!instructions.is_empty()).then(|| instructions.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::text_transform::{
        local_polish_timeout, should_reject_question_answer_polish, LOCAL_POLISH_BASE_TIMEOUT,
        LOCAL_POLISH_MAX_TIMEOUT,
    };
    fn polish_rejection_reason(
        input: &str,
        output: &str,
        template: Option<&str>,
    ) -> Option<&'static str> {
        crate::services::text_transform::rejection_reason(
            input,
            output,
            TransformIntent::for_template(template),
        )
    }

    #[test]
    fn rejects_polish_output_that_answers_a_dictated_question() {
        let input =
            "哎，你觉得这个功能现在完整了吗？咱们达到可以发布0.1版本的时候了吗？所有东西都就绪了吗？";
        let output =
            "我觉得这个功能现在还不够完整，还没到可以发布 0.1 版本的时候，还不是所有东西都就绪了。";

        assert!(should_reject_question_answer_polish(input, output));
    }

    #[test]
    fn accepts_polish_output_that_preserves_a_question() {
        let input = "看一下最终的结果，我们当前是不是可以发布0.1版本了？还差哪些东西？";
        let output = "看一下最终的结果，我们当前是不是可以发布 0.1 版本了？还差哪些东西？";

        assert!(!should_reject_question_answer_polish(input, output));
    }

    #[test]
    fn accepts_non_question_polish_output() {
        let input = "嗯，我觉得这个功能现在已经完整了";
        let output = "我觉得这个功能现在已经完整了。";

        assert!(!should_reject_question_answer_polish(input, output));
    }

    #[test]
    fn rejects_production_regression_that_changed_french_to_english() {
        let input = "Alors je suis en train de faire un test et j'aimerais que tu fasses trois choses pour moi. Premièrement, vérifie le tracking. Deuxièmement, vérifie la sécurité. Ensuite, regarde les nouvelles fonctionnalités que nous devons implémenter.";
        let output = "Task:\n\n- Fix login bug\n- Add error handling";

        assert_eq!(
            polish_rejection_reason(input, output, Some("agent")),
            Some("polish changed transcript language")
        );
    }

    #[test]
    fn rejects_production_regression_that_removed_most_content() {
        let input = "Je veux vérifier trois points distincts dans l'application. Le premier concerne le suivi des événements et les données envoyées. Le deuxième concerne les règles de sécurité. Le troisième concerne les nouvelles fonctionnalités à planifier pour la prochaine version.";
        let output = "Vérifier les points de l'application.";

        assert_eq!(
            polish_rejection_reason(input, output, Some("document")),
            Some("polish removed too much transcript content")
        );
    }

    #[test]
    fn accepts_french_cleanup_that_preserves_substance() {
        let input = "Alors euh je veux vérifier le tracking, puis je veux vérifier la sécurité, et ensuite je veux regarder les nouvelles fonctionnalités que nous devons implémenter dans la prochaine version.";
        let output = "Je veux vérifier le tracking et la sécurité, puis examiner les nouvelles fonctionnalités à implémenter dans la prochaine version.";

        assert_eq!(polish_rejection_reason(input, output, Some("filler")), None);
    }

    #[test]
    fn concise_template_may_shorten_without_discarding_everything() {
        let input = "Je pense que nous devrions probablement commencer par vérifier les événements de suivi, puis examiner les règles de sécurité, avant de décider quelles nouvelles fonctionnalités seront ajoutées à la prochaine version de l'application.";
        let output =
            "Vérifions le suivi et la sécurité avant de choisir les prochaines fonctionnalités.";

        assert_eq!(
            polish_rejection_reason(input, output, Some("concise")),
            None
        );
    }

    #[test]
    fn rejects_english_input_rewritten_in_french() {
        let input = "Please review the tracking events, check the security rules, and list the product features that should be implemented in the next release.";
        let output = "Vérifie les événements de suivi, les règles de sécurité et les prochaines fonctionnalités du produit.";

        assert_eq!(
            polish_rejection_reason(input, output, Some("filler")),
            Some("polish changed transcript language")
        );
    }

    #[test]
    fn accepts_language_change_for_an_explicit_translation_workflow() {
        let input = "Please review the tracking events, check the security rules, and list the product features that should be implemented in the next release.";
        let output = "Vérifie les événements de suivi, les règles de sécurité et les prochaines fonctionnalités du produit.";

        assert_eq!(
            crate::services::text_transform::rejection_reason(
                input,
                output,
                TransformIntent::Translate
            ),
            None
        );
    }

    #[test]
    fn workflow_profile_adds_translation_and_shared_code_aware_instruction() {
        let profile = crate::services::product_workflows::WorkflowProfile {
            id: "code-fr".to_string(),
            name: "Code French".to_string(),
            hotkey: "Cmd+1".to_string(),
            trigger_mode: crate::shortcut::ShortcutTriggerMode::Toggle,
            language: Some("en".to_string()),
            polish_template_id: None,
            translation_target: Some("French".to_string()),
            output_action: crate::services::product_workflows::OutputAction::Preview,
            code_aware: true,
            protected: false,
        };

        let instruction = workflow_profile_instruction(&profile).unwrap();

        assert!(instruction.contains("Translate the complete output to French"));
        assert!(instruction.contains("Preserve code identifiers and casing"));
        assert!(instruction.contains("Do not wrap the result in a Markdown code fence"));
    }

    #[test]
    fn local_polish_timeout_stays_short_for_short_text() {
        assert_eq!(
            local_polish_timeout("short text"),
            LOCAL_POLISH_BASE_TIMEOUT
        );
    }

    #[test]
    fn local_polish_timeout_expands_for_long_text() {
        let text = "a".repeat(1_400);
        let timeout = local_polish_timeout(&text);

        assert!(timeout > LOCAL_POLISH_BASE_TIMEOUT);
        assert!(timeout <= LOCAL_POLISH_MAX_TIMEOUT);
    }

    #[test]
    fn local_polish_timeout_is_capped() {
        let text = "a".repeat(20_000);

        assert_eq!(local_polish_timeout(&text), LOCAL_POLISH_MAX_TIMEOUT);
    }

    #[test]
    fn classifies_model_load_failure_as_memory_likely() {
        assert_eq!(
            classify_polish_failure_reason("Model load: null reference from llama.cpp"),
            "model could not be loaded, likely low memory"
        );
    }

    #[test]
    fn classifies_incomplete_download_failure() {
        assert_eq!(
            classify_polish_failure_reason(
                "Model file appears incomplete: 18MB (expected at least 400MB)"
            ),
            "model download looks incomplete"
        );
    }

    #[test]
    fn classifies_cloud_auth_and_rate_limit_failures() {
        assert_eq!(
            classify_polish_failure_reason("HTTP 401 unauthorized: bad API key"),
            "cloud polish authentication failed"
        );
        assert_eq!(
            classify_polish_failure_reason("HTTP 429 rate limit exceeded"),
            "cloud polish was rate limited"
        );
    }

    #[test]
    fn classifies_local_timeout_without_cloud_reason() {
        assert_eq!(
            classify_polish_failure_reason("Local polish timed out"),
            "local polish timed out"
        );
    }

    #[test]
    fn classifies_local_server_unavailable_without_cloud_reason() {
        assert_eq!(
            classify_polish_failure_reason("Local polish server unavailable: connection refused"),
            "local polish server unavailable"
        );
    }
}
