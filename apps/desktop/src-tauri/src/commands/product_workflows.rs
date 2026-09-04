use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::commands::settings::{self, AppSettings};
use crate::services::product_workflows::{
    build_voice_action_instruction, build_voice_action_preview, expand_matching_snippet,
    resolve_profile, ApplicationRule, CapturedContext, ContextCaptureSettings, DeliveryRecord,
    OutputAction, VoiceActionKind, VoiceActionPreview, VoiceSnippet, WorkflowProfile,
    WorkflowRuntime,
};
use crate::state::app_state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSettingsSnapshot {
    pub context_capture: ContextCaptureSettings,
    pub profiles: Vec<WorkflowProfile>,
    pub application_rules: Vec<ApplicationRule>,
    pub snippets: Vec<VoiceSnippet>,
}

impl From<&AppSettings> for WorkflowSettingsSnapshot {
    fn from(settings: &AppSettings) -> Self {
        Self {
            context_capture: settings.context_capture.clone(),
            profiles: settings.workflow_profiles.clone(),
            application_rules: settings.application_rules.clone(),
            snippets: settings.voice_snippets.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceActionRequest {
    pub kind: VoiceActionKind,
    pub selected_text: Option<String>,
    pub translation_target: Option<String>,
    pub custom_instruction: Option<String>,
    pub output_action: OutputAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickControlKind {
    UndoLastInsertion,
    ReinsertRaw,
    ReinsertFinal,
    CopyRaw,
    CopyFinal,
    Repolish,
    SubmitEnter,
    CancelActiveTask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickControlResult {
    pub action: QuickControlKind,
    pub text: Option<String>,
}

#[tauri::command]
pub fn get_workflow_settings(
    state: State<'_, AppState>,
) -> Result<WorkflowSettingsSnapshot, String> {
    Ok(WorkflowSettingsSnapshot::from(&*state.settings.lock()))
}

#[tauri::command]
pub async fn capture_workflow_context(
    app: AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, WorkflowRuntime>,
) -> Result<CapturedContext, String> {
    let settings = state.settings.lock().context_capture.clone();
    let clipboard_text = if settings.clipboard {
        app.clipboard().read_text().ok()
    } else {
        None
    };
    let context =
        crate::sensors::focused_context::capture_focused_context(&settings, clipboard_text).await;
    runtime.set_context(context.clone());
    Ok(context)
}

#[tauri::command]
pub fn get_latest_workflow_context(
    runtime: State<'_, WorkflowRuntime>,
) -> Result<Option<CapturedContext>, String> {
    Ok(runtime.context())
}

#[tauri::command]
pub fn resolve_workflow_profile(
    state: State<'_, AppState>,
    runtime: State<'_, WorkflowRuntime>,
    requested_profile_id: Option<String>,
) -> Result<WorkflowProfile, String> {
    let context = runtime.context().unwrap_or_default();
    let settings = state.settings.lock();
    resolve_profile(
        &settings.workflow_profiles,
        &settings.application_rules,
        requested_profile_id.as_deref(),
        context.application_id.as_deref(),
        context.window_title.as_deref(),
    )
    .cloned()
    .ok_or_else(|| "No workflow profile is available".to_string())
}

#[tauri::command]
pub fn create_workflow_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile: WorkflowProfile,
) -> Result<(), String> {
    let mut profiles = state.settings.lock().workflow_profiles.clone();
    if profiles.iter().any(|item| item.id == profile.id) {
        return Err(format!("Profile already exists: {}", profile.id));
    }
    profiles.push(profile);
    settings::update_settings(
        app,
        state,
        "workflow_profiles".to_string(),
        serde_json::to_value(profiles).map_err(|error| error.to_string())?,
    )
}

#[tauri::command]
pub fn update_workflow_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile: WorkflowProfile,
) -> Result<(), String> {
    let mut profiles = state.settings.lock().workflow_profiles.clone();
    let existing = profiles
        .iter_mut()
        .find(|item| item.id == profile.id)
        .ok_or_else(|| format!("Profile not found: {}", profile.id))?;
    let protected = existing.protected;
    *existing = WorkflowProfile {
        protected,
        ..profile
    };
    settings::update_settings(
        app,
        state,
        "workflow_profiles".to_string(),
        serde_json::to_value(profiles).map_err(|error| error.to_string())?,
    )
}

#[tauri::command]
pub fn delete_workflow_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    let settings_snapshot = state.settings.lock().clone();
    let profile = settings_snapshot
        .workflow_profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| format!("Profile not found: {profile_id}"))?;
    if profile.protected {
        return Err("The default profile cannot be deleted".to_string());
    }
    if settings_snapshot
        .application_rules
        .iter()
        .any(|rule| rule.profile_id == profile_id)
    {
        return Err("Delete application rules using this profile first".to_string());
    }
    let profiles = settings_snapshot
        .workflow_profiles
        .into_iter()
        .filter(|profile| profile.id != profile_id)
        .collect::<Vec<_>>();
    settings::update_settings(
        app,
        state,
        "workflow_profiles".to_string(),
        serde_json::to_value(profiles).map_err(|error| error.to_string())?,
    )
}

#[tauri::command]
pub fn set_application_rules(
    app: AppHandle,
    state: State<'_, AppState>,
    rules: Vec<ApplicationRule>,
) -> Result<(), String> {
    settings::update_settings(
        app,
        state,
        "application_rules".to_string(),
        serde_json::to_value(rules).map_err(|error| error.to_string())?,
    )
}

#[tauri::command]
pub fn upsert_application_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    rule: ApplicationRule,
) -> Result<(), String> {
    let mut rules = state.settings.lock().application_rules.clone();
    match rules.iter_mut().find(|item| item.id == rule.id) {
        Some(existing) => *existing = rule,
        None => rules.push(rule),
    }
    set_application_rules(app, state, rules)
}

#[tauri::command]
pub fn delete_application_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    rule_id: String,
) -> Result<(), String> {
    let mut rules = state.settings.lock().application_rules.clone();
    let previous_len = rules.len();
    rules.retain(|rule| rule.id != rule_id);
    if rules.len() == previous_len {
        return Err(format!("Application rule not found: {rule_id}"));
    }
    set_application_rules(app, state, rules)
}

#[tauri::command]
pub fn set_voice_snippets(
    app: AppHandle,
    state: State<'_, AppState>,
    snippets: Vec<VoiceSnippet>,
) -> Result<(), String> {
    settings::update_settings(
        app,
        state,
        "voice_snippets".to_string(),
        serde_json::to_value(snippets).map_err(|error| error.to_string())?,
    )
}

#[tauri::command]
pub fn upsert_voice_snippet(
    app: AppHandle,
    state: State<'_, AppState>,
    snippet: VoiceSnippet,
) -> Result<(), String> {
    let mut snippets = state.settings.lock().voice_snippets.clone();
    match snippets.iter_mut().find(|item| item.id == snippet.id) {
        Some(existing) => *existing = snippet,
        None => snippets.push(snippet),
    }
    set_voice_snippets(app, state, snippets)
}

#[tauri::command]
pub fn delete_voice_snippet(
    app: AppHandle,
    state: State<'_, AppState>,
    snippet_id: String,
) -> Result<(), String> {
    let mut snippets = state.settings.lock().voice_snippets.clone();
    let previous_len = snippets.len();
    snippets.retain(|snippet| snippet.id != snippet_id);
    if snippets.len() == previous_len {
        return Err(format!("Voice snippet not found: {snippet_id}"));
    }
    set_voice_snippets(app, state, snippets)
}

#[tauri::command]
pub fn set_context_capture_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings_value: ContextCaptureSettings,
) -> Result<(), String> {
    settings::update_settings(
        app,
        state,
        "context_capture".to_string(),
        serde_json::to_value(settings_value).map_err(|error| error.to_string())?,
    )
}

#[tauri::command]
pub fn expand_voice_snippet(
    state: State<'_, AppState>,
    runtime: State<'_, WorkflowRuntime>,
    spoken_text: String,
) -> Result<Option<String>, String> {
    let snippets = state.settings.lock().voice_snippets.clone();
    let context = runtime.context().unwrap_or_default();
    expand_matching_snippet(
        &snippets,
        &spoken_text,
        &context,
        &chrono::Local::now().format("%Y-%m-%d").to_string(),
    )
}

#[tauri::command]
pub async fn run_voice_action(
    app: AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, WorkflowRuntime>,
    request: VoiceActionRequest,
) -> Result<VoiceActionPreview, String> {
    let context = runtime.context().unwrap_or_default();
    let selected_text = request
        .selected_text
        .as_deref()
        .or(context.selected_text.as_deref());
    let instruction = build_voice_action_instruction(
        request.kind,
        request.translation_target.as_deref(),
        request.custom_instruction.as_deref(),
    )?;
    let source = selected_text
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "Selected text is required".to_string())?;
    let result = polish_text(&state, source, &instruction).await?;
    let mut preview = build_voice_action_preview(
        request.kind,
        Some(source),
        &result,
        request.translation_target,
    )?;
    preview.output_action = request.output_action;
    runtime.set_preview(preview.clone());

    match request.output_action {
        OutputAction::Preview => {}
        OutputAction::Copy => app
            .clipboard()
            .write_text(&preview.result_text)
            .map_err(|error| error.to_string())?,
        OutputAction::Insert => deliver_preview(&app, &runtime, &preview, context.application_id)?,
    }
    Ok(preview)
}

#[tauri::command]
pub fn replace_voice_action_preview(
    app: AppHandle,
    runtime: State<'_, WorkflowRuntime>,
) -> Result<VoiceActionPreview, String> {
    let preview = runtime
        .preview()
        .ok_or_else(|| "No voice action preview is available".to_string())?;
    let application_id = runtime.context().and_then(|context| context.application_id);
    deliver_preview(&app, &runtime, &preview, application_id)?;
    Ok(preview)
}

fn deliver_preview(
    app: &AppHandle,
    runtime: &WorkflowRuntime,
    preview: &VoiceActionPreview,
    application_id: Option<String>,
) -> Result<(), String> {
    let application_id = application_id
        .filter(|application_id| application_id != &app.config().identifier)
        .ok_or_else(|| "Capture the source application before replacing text".to_string())?;
    crate::sensors::focused_context::activate_application(&application_id)?;
    std::thread::sleep(Duration::from_millis(150));
    crate::commands::text::do_insert_text(&preview.result_text)?;
    runtime.record_delivery(DeliveryRecord {
        raw_text: preview.source_text.clone(),
        final_text: preview.result_text.clone(),
        inserted_text: preview.result_text.clone(),
        application_id: Some(application_id),
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        undone: false,
    });
    Ok(())
}

#[tauri::command]
pub fn record_workflow_delivery(
    runtime: State<'_, WorkflowRuntime>,
    raw_text: String,
    final_text: String,
    inserted_text: String,
    application_id: Option<String>,
) -> Result<(), String> {
    runtime.record_delivery(DeliveryRecord {
        raw_text,
        final_text,
        inserted_text,
        application_id,
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        undone: false,
    });
    Ok(())
}

#[tauri::command]
pub async fn run_quick_control(
    app: AppHandle,
    state: State<'_, AppState>,
    runtime: State<'_, WorkflowRuntime>,
    action: QuickControlKind,
) -> Result<QuickControlResult, String> {
    let text = match action {
        QuickControlKind::UndoLastInsertion => {
            let record = require_last_delivery(&runtime)?;
            activate_workflow_target(&app, record.application_id.as_deref())?;
            Some(undo_last_insertion(&runtime, || {
                crate::text_injector::quick_controls::send_undo()
            })?)
        }
        QuickControlKind::ReinsertRaw => {
            let record = require_last_delivery(&runtime)?;
            activate_workflow_target(&app, record.application_id.as_deref())?;
            crate::commands::text::do_insert_text(&record.raw_text)?;
            let text = record.raw_text.clone();
            record_reinsertion(&runtime, record, &text);
            Some(text)
        }
        QuickControlKind::ReinsertFinal => {
            let record = require_last_delivery(&runtime)?;
            activate_workflow_target(&app, record.application_id.as_deref())?;
            crate::commands::text::do_insert_text(&record.final_text)?;
            let text = record.final_text.clone();
            record_reinsertion(&runtime, record, &text);
            Some(text)
        }
        QuickControlKind::CopyRaw => {
            let text = require_last_delivery(&runtime)?.raw_text;
            app.clipboard()
                .write_text(&text)
                .map_err(|error| error.to_string())?;
            Some(text)
        }
        QuickControlKind::CopyFinal => {
            let text = require_last_delivery(&runtime)?.final_text;
            app.clipboard()
                .write_text(&text)
                .map_err(|error| error.to_string())?;
            Some(text)
        }
        QuickControlKind::Repolish => {
            let source = require_last_delivery(&runtime)?.raw_text;
            let result =
                polish_text(&state, &source, crate::polish_engine::DEFAULT_POLISH_PROMPT).await?;
            let preview =
                build_voice_action_preview(VoiceActionKind::Custom, Some(&source), &result, None)?;
            runtime.set_preview(preview);
            Some(result)
        }
        QuickControlKind::SubmitEnter => {
            let application_id = runtime.context().and_then(|context| context.application_id);
            activate_workflow_target(&app, application_id.as_deref())?;
            crate::text_injector::quick_controls::send_enter()?;
            None
        }
        QuickControlKind::CancelActiveTask => {
            let task_id = runtime
                .active_task_id()
                .ok_or_else(|| "No active task is available to cancel".to_string())?;
            crate::commands::audio::cancel_recording_sync(app.clone())?;
            runtime.clear_active_task(task_id);
            None
        }
    };
    Ok(QuickControlResult { action, text })
}

fn require_last_delivery(runtime: &WorkflowRuntime) -> Result<DeliveryRecord, String> {
    runtime
        .last_delivery()
        .ok_or_else(|| "No previous delivery is available".to_string())
}

fn record_reinsertion(runtime: &WorkflowRuntime, previous: DeliveryRecord, inserted_text: &str) {
    runtime.record_delivery(DeliveryRecord {
        raw_text: previous.raw_text,
        final_text: previous.final_text,
        inserted_text: inserted_text.to_string(),
        application_id: runtime
            .context()
            .and_then(|context| context.application_id)
            .or(previous.application_id),
        created_at_ms: chrono::Utc::now().timestamp_millis(),
        undone: false,
    });
}

fn activate_workflow_target(app: &AppHandle, application_id: Option<&str>) -> Result<(), String> {
    let Some(application_id) = application_id else {
        return Ok(());
    };
    if application_id == app.config().identifier {
        return Err("The source application is Voice Flow".to_string());
    }
    crate::sensors::focused_context::activate_application(application_id)?;
    std::thread::sleep(Duration::from_millis(150));
    Ok(())
}

fn undo_last_insertion(
    runtime: &WorkflowRuntime,
    send_undo: impl FnOnce() -> Result<(), String>,
) -> Result<String, String> {
    let inserted_text = runtime.pending_undo_text()?;
    send_undo()?;
    runtime.mark_undone()?;
    Ok(inserted_text)
}

async fn polish_text(state: &AppState, text: &str, instruction: &str) -> Result<String, String> {
    let (language, cloud_enabled, provider_type, cloud_config, local_model_id) = {
        let settings = state.settings.lock();
        (
            settings.stt_engine_language.clone(),
            settings.cloud_polish_enabled,
            settings.active_cloud_polish_provider.clone(),
            settings
                .cloud_polish_configs
                .get(&settings.active_cloud_polish_provider)
                .cloned(),
            settings.polish_model.clone(),
        )
    };
    let request = crate::polish_engine::PolishRequest::new(text, instruction, language)
        .with_timeout(Duration::from_secs(60));

    let result = if cloud_enabled {
        let config = cloud_config.ok_or_else(|| "Cloud polish is not configured".to_string())?;
        if config.api_key.trim().is_empty() || config.model.trim().is_empty() {
            return Err("Cloud polish credentials and model are required".to_string());
        }
        state
            .polish_manager
            .polish_cloud(
                request,
                &provider_type,
                &config.api_key,
                &config.base_url,
                &config.model,
                config.enable_thinking,
            )
            .await?
    } else {
        let engine =
            crate::polish_engine::UnifiedPolishManager::get_engine_by_model_id(&local_model_id)
                .ok_or_else(|| "Select a local polish model first".to_string())?;
        let filename = state
            .polish_manager
            .get_model_filename(engine, &local_model_id)
            .ok_or_else(|| "The selected local polish model is unknown".to_string())?;
        if !state
            .polish_manager
            .is_model_downloaded(engine, &local_model_id)
        {
            return Err("The selected local polish model is not downloaded".to_string());
        }
        state
            .polish_manager
            .polish(engine, request.with_model(filename))
            .await?
    };

    let result = result.text.trim().to_string();
    if result.is_empty() {
        Err("Polish returned empty text".to_string())
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_snapshot_contains_all_headless_configuration() {
        let settings = AppSettings::default();
        let snapshot = WorkflowSettingsSnapshot::from(&settings);

        assert_eq!(snapshot.profiles.len(), 1);
        assert_eq!(snapshot.profiles[0].id, "dictate");
        assert!(snapshot.application_rules.is_empty());
        assert!(snapshot.snippets.is_empty());
        assert!(!snapshot.context_capture.clipboard);
        assert!(!snapshot.context_capture.ocr_fallback);
    }

    #[test]
    fn quick_control_names_have_stable_snake_case_contract() {
        assert_eq!(
            serde_json::to_string(&QuickControlKind::UndoLastInsertion).unwrap(),
            "\"undo_last_insertion\""
        );
        assert_eq!(
            serde_json::to_string(&QuickControlKind::CancelActiveTask).unwrap(),
            "\"cancel_active_task\""
        );
    }

    #[test]
    fn failed_platform_undo_keeps_journal_retryable() {
        let runtime = WorkflowRuntime::default();
        runtime.record_delivery(DeliveryRecord {
            raw_text: "raw".to_string(),
            final_text: "final".to_string(),
            inserted_text: "final".to_string(),
            application_id: None,
            created_at_ms: 1,
            undone: false,
        });

        assert!(undo_last_insertion(&runtime, || Err("keyboard failed".to_string())).is_err());
        assert_eq!(undo_last_insertion(&runtime, || Ok(())).unwrap(), "final");
        assert!(undo_last_insertion(&runtime, || Ok(())).is_err());
    }
}
