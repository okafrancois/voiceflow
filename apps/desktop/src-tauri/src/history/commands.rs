use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::models::{
    HistoryFilter, HistoryStatistics, NewTranscriptionEntry, StatisticsPeriod, TranscriptionEntry,
};
use super::store::{EntryUpdates, HistoryStore, WorkbenchEntryUpdates};
use crate::services::transcription_workbench::{
    self, ExportFormat, FileTranscriptionRequest, FileTranscriptionResult, LanguageMode,
};
use crate::state::app_state::AppState;
use crate::stt_engine::traits::TranscriptionRequest;

const MIN_FAILED_RECORDING_DURATION_MS: i64 = 500;
const FILE_JOB_EVENT: &str = "file-transcription-job-changed";
const MAX_TERMINAL_FILE_JOBS: usize = 100;

static FILE_TRANSCRIPTION_JOBS: LazyLock<parking_lot::Mutex<HashMap<String, Arc<FileJobRecord>>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashMap::new()));
static NEXT_FILE_JOB_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[tauri::command]
pub fn get_transcription_history(
    state: State<'_, AppState>,
    filter: HistoryFilter,
) -> Result<Vec<TranscriptionEntry>, String> {
    let store = state.history_store.lock();
    store.get_history(&filter)
}

#[tauri::command]
pub fn get_transcription_entry(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<TranscriptionEntry>, String> {
    let store = state.history_store.lock();
    store.get_entry(&id)
}

#[tauri::command]
pub fn get_retention_status(
    state: State<'_, AppState>,
) -> Result<super::store::RetentionStatus, String> {
    let store = state.history_store.lock();
    store.get_retention_status()
}

#[tauri::command]
pub fn delete_transcription_entry(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let store = state.history_store.lock();
    store.delete_entry(&id)
}

#[tauri::command]
pub fn clear_transcription_history(state: State<'_, AppState>) -> Result<(), String> {
    let store = state.history_store.lock();
    store.clear_all()
}

#[tauri::command]
pub fn get_history_count(state: State<'_, AppState>, filter: HistoryFilter) -> Result<i64, String> {
    let store = state.history_store.lock();
    store.get_count(&filter)
}

#[tauri::command]
pub fn get_history_statistics(
    state: State<'_, AppState>,
    period: StatisticsPeriod,
) -> Result<HistoryStatistics, String> {
    state.history_store.lock().get_history_statistics(period)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryTextVersion {
    Raw,
    Final,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoryAudioPayload {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
struct ProcessedMedia {
    raw_text: String,
    final_text: String,
    source_path: PathBuf,
    language: Option<String>,
    translation_target: Option<String>,
    audio_duration_ms: i64,
    stt_duration_ms: i64,
    polish_duration_ms: Option<i64>,
    polish_engine: Option<String>,
    stt_engine: String,
    stt_model: String,
    output_action: crate::services::product_workflows::OutputAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileJobState {
    Queued,
    Running,
    Completed,
    Error,
    Canceled,
}

impl FileJobState {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Error | Self::Canceled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTranscriptionJob {
    pub id: String,
    pub state: FileJobState,
    pub progress_percent: u8,
    pub request: FileTranscriptionRequest,
    pub result: Option<FileTranscriptionResult>,
    pub error: Option<String>,
}

#[derive(Debug)]
struct FileJobRecord {
    snapshot: parking_lot::Mutex<FileTranscriptionJob>,
    canceled: AtomicBool,
    sequence: u64,
}

impl FileJobRecord {
    fn new(request: FileTranscriptionRequest) -> Self {
        Self {
            snapshot: parking_lot::Mutex::new(FileTranscriptionJob {
                id: uuid::Uuid::new_v4().to_string(),
                state: FileJobState::Queued,
                progress_percent: 0,
                request,
                result: None,
                error: None,
            }),
            canceled: AtomicBool::new(false),
            sequence: NEXT_FILE_JOB_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        }
    }

    fn snapshot(&self) -> FileTranscriptionJob {
        self.snapshot.lock().clone()
    }

    fn update(&self, state: FileJobState, progress_percent: u8) -> FileTranscriptionJob {
        let mut snapshot = self.snapshot.lock();
        if snapshot.state != FileJobState::Canceled {
            snapshot.state = state;
            snapshot.progress_percent = progress_percent.min(100);
        }
        snapshot.clone()
    }

    fn cancel(&self) -> Result<FileTranscriptionJob, String> {
        let mut snapshot = self.snapshot.lock();
        if snapshot.state.is_terminal() {
            return Err(format!(
                "File transcription job is already terminal: {}",
                snapshot.id
            ));
        }
        self.canceled.store(true, Ordering::SeqCst);
        snapshot.state = FileJobState::Canceled;
        snapshot.error = None;
        Ok(snapshot.clone())
    }
}

fn prune_terminal_file_jobs(
    jobs: &mut HashMap<String, Arc<FileJobRecord>>,
    maximum_terminal_jobs: usize,
) {
    let mut terminal_jobs = jobs
        .iter()
        .filter(|(_, record)| record.snapshot().state.is_terminal())
        .map(|(id, record)| (record.sequence, id.clone()))
        .collect::<Vec<_>>();
    terminal_jobs.sort_by_key(|(sequence, _)| *sequence);
    let remove_count = terminal_jobs.len().saturating_sub(maximum_terminal_jobs);
    for (_, id) in terminal_jobs.into_iter().take(remove_count) {
        jobs.remove(&id);
    }
}

#[tauri::command]
pub fn select_media_file() -> Result<Option<String>, String> {
    transcription_workbench::pick_media_file()
        .map(|path| path.map(|path| path.display().to_string()))
}

#[tauri::command]
pub fn select_export_file(format: ExportFormat) -> Result<Option<String>, String> {
    transcription_workbench::pick_export_file(format)
        .map(|path| path.map(|path| path.display().to_string()))
}

#[tauri::command]
pub async fn transcribe_media_file(
    app: AppHandle,
    state: State<'_, AppState>,
    request: FileTranscriptionRequest,
) -> Result<FileTranscriptionResult, String> {
    let processed = process_media_file(&state, &request).await?;
    finalize_processed_media(&app, &state, processed)
}

#[tauri::command]
pub fn start_file_transcription_job(
    app: AppHandle,
    request: FileTranscriptionRequest,
) -> Result<FileTranscriptionJob, String> {
    transcription_workbench::validate_media_path(&request.path)?;
    let record = Arc::new(FileJobRecord::new(request));
    let snapshot = record.snapshot();
    {
        let mut jobs = FILE_TRANSCRIPTION_JOBS.lock();
        prune_terminal_file_jobs(&mut jobs, MAX_TERMINAL_FILE_JOBS);
        jobs.insert(snapshot.id.clone(), record.clone());
    }
    emit_file_job(&app, &snapshot);

    tauri::async_runtime::spawn(async move {
        let running = record.update(FileJobState::Running, 5);
        emit_file_job(&app, &running);
        if record.canceled.load(Ordering::SeqCst) {
            return;
        }
        let request = record.snapshot().request;
        let state = app.state::<AppState>();
        let outcome = process_media_file(&state, &request).await;
        if record.canceled.load(Ordering::SeqCst) {
            return;
        }
        let result =
            outcome.and_then(|processed| finalize_processed_media(&app, &state, processed));
        let snapshot = match result {
            Ok(result) => {
                let mut snapshot = record.snapshot.lock();
                snapshot.state = FileJobState::Completed;
                snapshot.progress_percent = 100;
                snapshot.result = Some(result);
                snapshot.error = None;
                snapshot.clone()
            }
            Err(error) => {
                let mut snapshot = record.snapshot.lock();
                snapshot.state = FileJobState::Error;
                snapshot.progress_percent = 100;
                snapshot.error = Some(error);
                snapshot.clone()
            }
        };
        emit_file_job(&app, &snapshot);
        prune_terminal_file_jobs(&mut FILE_TRANSCRIPTION_JOBS.lock(), MAX_TERMINAL_FILE_JOBS);
    });

    Ok(snapshot)
}

#[tauri::command]
pub fn get_file_transcription_job(id: String) -> Result<FileTranscriptionJob, String> {
    FILE_TRANSCRIPTION_JOBS
        .lock()
        .get(&id)
        .map(|record| record.snapshot())
        .ok_or_else(|| format!("File transcription job not found: {id}"))
}

#[tauri::command]
pub fn list_file_transcription_jobs() -> Vec<FileTranscriptionJob> {
    let mut jobs = FILE_TRANSCRIPTION_JOBS
        .lock()
        .values()
        .map(|record| record.snapshot())
        .collect::<Vec<_>>();
    jobs.sort_by(|left, right| left.id.cmp(&right.id));
    jobs
}

#[tauri::command]
pub fn cancel_file_transcription_job(
    app: AppHandle,
    id: String,
) -> Result<FileTranscriptionJob, String> {
    let record = FILE_TRANSCRIPTION_JOBS
        .lock()
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("File transcription job not found: {id}"))?;
    let snapshot = record.cancel()?;
    emit_file_job(&app, &snapshot);
    prune_terminal_file_jobs(&mut FILE_TRANSCRIPTION_JOBS.lock(), MAX_TERMINAL_FILE_JOBS);
    Ok(snapshot)
}

fn emit_file_job(app: &AppHandle, snapshot: &FileTranscriptionJob) {
    if let Err(error) = app.emit(FILE_JOB_EVENT, snapshot) {
        tracing::warn!(error = %error, job_id = %snapshot.id, "file_transcription_job_event_failed");
    }
}

fn finalize_processed_media(
    app: &AppHandle,
    state: &AppState,
    processed: ProcessedMedia,
) -> Result<FileTranscriptionResult, String> {
    let (output_action, delivery_status) = match processed.output_action {
        crate::services::product_workflows::OutputAction::Preview => {
            ("preview", "not_delivered".to_string())
        }
        crate::services::product_workflows::OutputAction::Insert => {
            let method = crate::text_injector::insert_text(&processed.final_text)?;
            let status = match method {
                crate::text_injector::InjectionMethod::Keyboard => "inserted_keyboard",
                crate::text_injector::InjectionMethod::Clipboard => "inserted_clipboard",
                crate::text_injector::InjectionMethod::Accessibility => "inserted_accessibility",
            };
            ("insert", status.to_string())
        }
        crate::services::product_workflows::OutputAction::Copy => {
            app.clipboard()
                .write_text(&processed.final_text)
                .map_err(|error| format!("Failed to copy file transcription: {error}"))?;
            ("copy", "copied".to_string())
        }
    };
    let text_retention = state.settings.lock().text_retention;
    let history_entry_id = if text_retention.retains_new_data() {
        let store = state.history_store.lock();
        Some(store.insert(NewTranscriptionEntry {
            raw_text: processed.raw_text.clone(),
            final_text: processed.final_text.clone(),
            stt_engine: processed.stt_engine.clone(),
            stt_model: Some(processed.stt_model.clone()),
            language: processed.language.clone(),
            audio_duration_ms: Some(processed.audio_duration_ms),
            stt_duration_ms: Some(processed.stt_duration_ms),
            polish_duration_ms: processed.polish_duration_ms,
            total_duration_ms: Some(
                processed.stt_duration_ms + processed.polish_duration_ms.unwrap_or_default(),
            ),
            polish_applied: processed.polish_duration_ms.is_some(),
            polish_engine: processed.polish_engine.clone(),
            is_cloud: false,
            audio_path: None,
            status: "success".to_string(),
            error: None,
            source_kind: "file".to_string(),
            source_path: Some(processed.source_path.display().to_string()),
            translation_target: processed.translation_target.clone(),
            timed_segments: Vec::new(),
            delivery_status: delivery_status.clone(),
        })?)
    } else {
        None
    };

    Ok(FileTranscriptionResult {
        history_entry_id,
        raw_text: processed.raw_text,
        final_text: processed.final_text,
        source_path: processed.source_path,
        translation_target: processed.translation_target,
        output_action: output_action.to_string(),
        delivery_status,
    })
}

#[tauri::command]
pub async fn retranscribe_history_entry(
    state: State<'_, AppState>,
    id: String,
) -> Result<TranscriptionEntry, String> {
    let entry = {
        let store = state.history_store.lock();
        store
            .get_entry(&id)?
            .ok_or_else(|| format!("History entry not found: {id}"))?
    };
    let source_path = entry
        .source_path
        .clone()
        .or(entry.audio_path.clone())
        .ok_or_else(|| "No retained media is available for retranscription".to_string())?;
    let request = FileTranscriptionRequest {
        path: PathBuf::from(source_path),
        profile_id: None,
        translation_target: entry.translation_target,
    };
    let processed = process_media_file(&state, &request).await?;

    let store = state.history_store.lock();
    store.update_workbench_entry(
        &id,
        WorkbenchEntryUpdates {
            raw_text: processed.raw_text,
            final_text: processed.final_text,
            stt_engine: processed.stt_engine,
            stt_model: Some(processed.stt_model),
            language: processed.language,
            audio_duration_ms: Some(processed.audio_duration_ms),
            stt_duration_ms: Some(processed.stt_duration_ms),
            polish_duration_ms: processed.polish_duration_ms,
            total_duration_ms: Some(
                processed.stt_duration_ms + processed.polish_duration_ms.unwrap_or_default(),
            ),
            polish_applied: processed.polish_duration_ms.is_some(),
            polish_engine: processed.polish_engine,
            is_cloud: false,
            translation_target: processed.translation_target,
            timed_segments: Vec::new(),
        },
    )?;
    store
        .get_entry(&id)?
        .ok_or_else(|| format!("History entry not found after retranscription: {id}"))
}

#[tauri::command]
pub async fn repolish_history_entry(
    state: State<'_, AppState>,
    id: String,
    template_id: Option<String>,
    translation_target: Option<String>,
) -> Result<TranscriptionEntry, String> {
    let entry = {
        let store = state.history_store.lock();
        store
            .get_entry(&id)?
            .ok_or_else(|| format!("History entry not found: {id}"))?
    };
    if entry.raw_text.trim().is_empty() {
        return Err("Raw transcription is empty".to_string());
    }
    let (final_text, duration_ms, engine) = polish_workbench_text(
        &state,
        &entry.raw_text,
        entry.language.as_deref(),
        translation_target.as_deref(),
        template_id.as_deref(),
        false,
    )
    .await?;

    let store = state.history_store.lock();
    store.update_repolished_text(
        &id,
        &final_text,
        duration_ms,
        &engine,
        translation_target.as_deref(),
    )?;
    store
        .get_entry(&id)?
        .ok_or_else(|| format!("History entry not found after polish: {id}"))
}

#[tauri::command]
pub fn export_history_entry(
    state: State<'_, AppState>,
    id: String,
    format: ExportFormat,
    output_path: String,
    overwrite: bool,
) -> Result<String, String> {
    let store = state.history_store.lock();
    let entry = store
        .get_entry(&id)?
        .ok_or_else(|| format!("History entry not found: {id}"))?;
    drop(store);
    let contents = transcription_workbench::render_export(
        format,
        &entry.raw_text,
        &entry.final_text,
        &entry.timed_segments,
        entry.audio_duration_ms,
    )?;
    transcription_workbench::write_export_file(Path::new(&output_path), &contents, overwrite)
        .map(|path| path.display().to_string())
}

#[tauri::command]
pub fn get_history_audio(
    state: State<'_, AppState>,
    id: String,
) -> Result<HistoryAudioPayload, String> {
    let entry = {
        let store = state.history_store.lock();
        store
            .get_entry(&id)?
            .ok_or_else(|| format!("History entry not found: {id}"))?
    };
    let path = entry
        .audio_path
        .or(entry.source_path)
        .ok_or_else(|| "No audio is available for this history entry".to_string())?;
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err(format!("History media file not found: {}", path.display()));
    }
    let mime_type = transcription_workbench::validate_playback_audio(&path)?.to_string();
    let bytes =
        std::fs::read(&path).map_err(|error| format!("Failed to read history media: {error}"))?;
    Ok(HistoryAudioPayload { mime_type, bytes })
}

#[tauri::command]
pub fn copy_history_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version: HistoryTextVersion,
) -> Result<(), String> {
    let entry = {
        let store = state.history_store.lock();
        store
            .get_entry(&id)?
            .ok_or_else(|| format!("History entry not found: {id}"))?
    };
    let text = match version {
        HistoryTextVersion::Raw => entry.raw_text,
        HistoryTextVersion::Final => entry.final_text,
    };
    if text.trim().is_empty() {
        return Err("Selected history text is empty".to_string());
    }
    app.clipboard()
        .write_text(&text)
        .map_err(|error| format!("Failed to copy history text: {error}"))?;
    state
        .history_store
        .lock()
        .update_delivery_status(&id, "copied")
}

#[tauri::command]
pub fn reinsert_history_entry(
    state: State<'_, AppState>,
    id: String,
    version: HistoryTextVersion,
) -> Result<String, String> {
    let entry = {
        let store = state.history_store.lock();
        store
            .get_entry(&id)?
            .ok_or_else(|| format!("History entry not found: {id}"))?
    };
    let text = match version {
        HistoryTextVersion::Raw => entry.raw_text,
        HistoryTextVersion::Final => entry.final_text,
    };
    if text.trim().is_empty() {
        return Err("Selected history text is empty".to_string());
    }
    match crate::text_injector::insert_text(&text) {
        Ok(method) => {
            let status = match method {
                crate::text_injector::InjectionMethod::Keyboard => "inserted_keyboard",
                crate::text_injector::InjectionMethod::Clipboard => "inserted_clipboard",
                crate::text_injector::InjectionMethod::Accessibility => "inserted_accessibility",
            };
            state
                .history_store
                .lock()
                .update_delivery_status(&id, status)?;
            Ok(status.to_string())
        }
        Err(error) => {
            if let Err(update_error) = state
                .history_store
                .lock()
                .update_delivery_status(&id, "failed")
            {
                tracing::warn!(error = %update_error, entry_id = %id, "history_delivery_status_update_failed");
            }
            Err(error)
        }
    }
}

fn paste_last_transcription_with<F>(store: &HistoryStore, insert: F) -> Result<String, String>
where
    F: FnOnce(&str) -> Result<crate::text_injector::InjectionMethod, String>,
{
    let latest = store
        .get_latest_successful_transcription()?
        .ok_or_else(|| "No successful transcription is available".to_string())?;

    match insert(&latest.final_text) {
        Ok(method) => {
            let status = match method {
                crate::text_injector::InjectionMethod::Keyboard => "inserted_keyboard",
                crate::text_injector::InjectionMethod::Clipboard => "inserted_clipboard",
                crate::text_injector::InjectionMethod::Accessibility => "inserted_accessibility",
            };
            store.update_delivery_status(&latest.id, status)?;
            Ok(status.to_string())
        }
        Err(error) => {
            if let Err(update_error) = store.update_delivery_status(&latest.id, "failed") {
                tracing::warn!(
                    error = %update_error,
                    entry_id = %latest.id,
                    "history_delivery_status_update_failed"
                );
            }
            Err(error)
        }
    }
}

pub fn paste_last_transcription_from_state(state: &AppState) -> Result<String, String> {
    let store = state.history_store.lock();
    paste_last_transcription_with(&store, crate::text_injector::insert_text)
}

#[tauri::command]
#[tracing::instrument(skip(state), ret, err)]
pub fn paste_last_transcription(state: State<'_, AppState>) -> Result<String, String> {
    paste_last_transcription_from_state(&state)
}

async fn process_media_file(
    state: &AppState,
    request: &FileTranscriptionRequest,
) -> Result<ProcessedMedia, String> {
    let media = transcription_workbench::validate_media_path(&request.path)?;
    let (
        requested_model,
        configured_language,
        translation_target,
        polish_template_id,
        output_action,
        code_aware,
    ) = {
        let settings = state.settings.lock();
        let profile = match request.profile_id.as_deref() {
            Some(profile_id) => Some(
                settings
                    .workflow_profiles
                    .iter()
                    .find(|profile| profile.id == profile_id)
                    .cloned()
                    .ok_or_else(|| format!("Workflow profile not found: {profile_id}"))?,
            ),
            None => None,
        };
        (
            settings.model.clone(),
            profile
                .as_ref()
                .and_then(|profile| profile.language.clone())
                .unwrap_or_else(|| settings.stt_engine_language.clone()),
            request.translation_target.clone().or_else(|| {
                profile
                    .as_ref()
                    .and_then(|profile| profile.translation_target.clone())
            }),
            profile
                .as_ref()
                .and_then(|profile| profile.polish_template_id.clone()),
            profile
                .as_ref()
                .map(|profile| profile.output_action)
                .unwrap_or(crate::services::product_workflows::OutputAction::Preview),
            profile.as_ref().is_some_and(|profile| profile.code_aware),
        )
    };
    let language = normalized_language(&configured_language);
    let language_policy = transcription_workbench::build_language_policy(
        language.as_deref(),
        translation_target.as_deref(),
    )?;
    let decoded = transcription_workbench::decode_media_to_mono_16k(&media.path)?;
    let audio_duration_ms =
        ((decoded.samples.len() as u128 * 1_000) / u128::from(decoded.sample_rate)) as i64;
    let (engine_type, model_name) = state
        .engine_manager
        .resolve_available_model(&requested_model, &configured_language);
    let mut transcription_request =
        TranscriptionRequest::new(decoded.samples).with_model(model_name.clone());
    if let Some(language) = language.as_deref() {
        transcription_request = transcription_request.with_language(language);
    }
    let transcription = state
        .engine_manager
        .transcribe(engine_type, transcription_request)
        .await
        .map_err(|error| format!("File transcription failed: {error}"))?;
    let raw_text = transcription.text.trim().to_string();
    if raw_text.is_empty() {
        return Err("File transcription returned empty text".to_string());
    }

    let should_polish = language_policy.mode == LanguageMode::Translate
        || polish_template_id.is_some()
        || code_aware;
    let (final_text, polish_duration_ms, polish_engine) = if should_polish {
        let (text, duration_ms, engine) = polish_workbench_text(
            state,
            &raw_text,
            language.as_deref(),
            translation_target.as_deref(),
            polish_template_id.as_deref(),
            code_aware,
        )
        .await?;
        (text, Some(duration_ms), Some(engine))
    } else {
        (raw_text.clone(), None, None)
    };

    Ok(ProcessedMedia {
        raw_text,
        final_text,
        source_path: media.path,
        language,
        translation_target: language_policy.target_language,
        audio_duration_ms,
        stt_duration_ms: transcription.total_ms as i64,
        polish_duration_ms,
        polish_engine,
        stt_engine: transcription.engine.as_str().to_string(),
        stt_model: model_name,
        output_action,
    })
}

async fn polish_workbench_text(
    state: &AppState,
    text: &str,
    source_language: Option<&str>,
    translation_target: Option<&str>,
    template_id: Option<&str>,
    code_aware: bool,
) -> Result<(String, i64, String), String> {
    let language_policy =
        transcription_workbench::build_language_policy(source_language, translation_target)?;
    let template_prompt = resolve_workbench_template_prompt(&state.settings.lock(), template_id)?;
    let mut system_prompt = if language_policy.mode == LanguageMode::Translate {
        format!(
            "{}\nOutput only the translated text. Do not explain the translation.",
            language_policy.instruction
        )
    } else {
        format!("{template_prompt}\n\n{}", language_policy.instruction)
    };
    if code_aware {
        system_prompt.push_str(
            "\nPreserve identifiers, command flags, file paths, symbols, and their exact casing unless the transcription is clearly wrong.",
        );
    }
    let output_language = language_policy
        .target_language
        .as_deref()
        .or(source_language)
        .unwrap_or("auto");
    let request = crate::polish_engine::PolishRequest::new(text, system_prompt, output_language);

    let intent = if language_policy.mode == LanguageMode::Translate {
        crate::services::text_transform::TransformIntent::Translate
    } else {
        crate::services::text_transform::TransformIntent::for_template(template_id)
    };
    let outcome = crate::services::text_transform::transform_text(state, request, intent).await?;
    let result = outcome.result;
    let output = result.text.clone();

    Ok((
        output,
        result.total_ms as i64,
        result.engine.as_str().to_string(),
    ))
}

fn resolve_workbench_template_prompt(
    settings: &crate::commands::settings::AppSettings,
    template_id: Option<&str>,
) -> Result<String, String> {
    if let Some(template_id) = template_id {
        return crate::polish_engine::get_template_by_id(template_id)
            .map(|template| template.system_prompt.to_string())
            .or_else(|| {
                settings
                    .polish_custom_templates
                    .iter()
                    .find(|template| template.id == template_id)
                    .map(|template| template.system_prompt.clone())
            })
            .ok_or_else(|| format!("Polish template not found: {template_id}"));
    }

    Ok(crate::polish_engine::get_template_by_id("filler")
        .map(|template| template.system_prompt.to_string())
        .unwrap_or_else(|| crate::polish_engine::DEFAULT_POLISH_PROMPT.to_string()))
}

fn normalized_language(language: &str) -> Option<String> {
    let language = language.trim();
    (!language.is_empty() && language != "auto").then(|| language.to_string())
}

/// Insert a history entry. Called internally from the transcription pipeline — not exposed to frontend.
pub fn save_history_entry(
    store: &HistoryStore,
    entry: NewTranscriptionEntry,
) -> Result<String, String> {
    store.insert(entry)
}

/// Update a history entry after retry. Called internally.
pub fn update_history_entry(
    store: &HistoryStore,
    id: &str,
    updates: EntryUpdates,
) -> Result<(), String> {
    store.update_entry(id, updates)
}

/// Mark a history entry as failed. Called internally.
pub fn mark_history_error(store: &HistoryStore, id: &str, error: &str) -> Result<(), String> {
    store.mark_error(id, error)
}

/// Retry transcription for a failed entry.
/// This is a Tauri command exposed to frontend.
#[tauri::command]
pub async fn retry_transcription(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    crate::commands::audio::retry_transcription_internal(app, state, id).await
}

/// Save a successful recording entry to history.
pub struct HistorySaveRequest<'a> {
    pub raw_text: &'a str,
    pub final_text: &'a str,
    pub stt_duration_ms: Option<i64>,
    pub polish_duration_ms: Option<i64>,
    pub polish_applied: bool,
    pub audio_path: Option<String>,
    pub delivery_status: &'a str,
}

pub fn save_to_history(
    state: &AppState,
    raw_text: &str,
    final_text: &str,
    stt_duration_ms: Option<i64>,
    polish_duration_ms: Option<i64>,
    polish_applied: bool,
    audio_path: Option<String>,
) -> Option<String> {
    save_to_history_with_delivery_status(
        state,
        HistorySaveRequest {
            raw_text,
            final_text,
            stt_duration_ms,
            polish_duration_ms,
            polish_applied,
            audio_path,
            delivery_status: "pending_insertion",
        },
    )
}

pub fn save_to_history_with_delivery_status(
    state: &AppState,
    request: HistorySaveRequest<'_>,
) -> Option<String> {
    let HistorySaveRequest {
        raw_text,
        final_text,
        stt_duration_ms,
        polish_duration_ms,
        polish_applied,
        audio_path,
        delivery_status,
    } = request;
    let (stt_engine, stt_model, language, is_cloud, text_retention, audio_retention) = {
        let settings = state.settings.lock();
        let cloud_config = settings.get_active_cloud_stt_config();
        let is_cloud = cloud_config.enabled;
        let engine_str = if is_cloud {
            format!("cloud-{}", cloud_config.provider_type)
        } else {
            crate::stt_engine::UnifiedEngineManager::get_engine_by_model_name(&settings.model)
                .map(|et| et.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        };
        (
            engine_str,
            if is_cloud {
                Some(cloud_config.model.clone())
            } else {
                Some(settings.model.clone())
            },
            if settings.stt_engine_language.is_empty() {
                None
            } else {
                Some(settings.stt_engine_language.clone())
            },
            is_cloud,
            settings.text_retention,
            settings.audio_retention,
        )
    };

    let recording_duration_ms = {
        let start = state
            .recording_start_time
            .load(std::sync::atomic::Ordering::SeqCst);
        if start > 0 {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64
                    - start as i64,
            )
        } else {
            None
        }
    };

    let total_ms = match (stt_duration_ms, polish_duration_ms) {
        (Some(stt), Some(pol)) => Some(stt + pol),
        (Some(stt), None) => Some(stt),
        _ => None,
    };

    let store = state.history_store.lock();
    let retained_audio_path = apply_audio_retention(&store, audio_path, audio_retention);
    if !text_retention.retains_new_data() {
        if let Some(path) = retained_audio_path {
            if let Err(error) =
                store.register_audio_asset(&path, chrono::Utc::now().timestamp_millis())
            {
                tracing::warn!(error = %error, path = %path, "audio_only_asset_registration_failed");
            }
        }
        return None;
    }

    let entry = NewTranscriptionEntry {
        raw_text: raw_text.to_string(),
        final_text: final_text.to_string(),
        stt_engine,
        stt_model,
        language,
        audio_duration_ms: recording_duration_ms,
        stt_duration_ms,
        polish_duration_ms,
        total_duration_ms: total_ms,
        polish_applied,
        polish_engine: None,
        is_cloud,
        audio_path: retained_audio_path,
        status: "success".to_string(),
        error: None,
        source_kind: "recording".to_string(),
        source_path: None,
        translation_target: None,
        timed_segments: Vec::new(),
        delivery_status: delivery_status.to_string(),
    };

    match save_history_entry(&store, entry) {
        Ok(id) => Some(id),
        Err(error) => {
            tracing::warn!(error = %error, "failed_to_save_history");
            None
        }
    }
}

/// Save a failed recording entry to history.
/// Only saves if recording duration >= 500ms to avoid noise from accidental short recordings.
pub fn save_failed_history(state: &AppState, audio_path: Option<String>, error: &str) {
    save_failed_history_with_duration_gate(state, audio_path, error, true);
}

/// Save a failed recording entry even when the failure happens before enough audio is captured.
/// This is used for infrastructure failures such as cloud websocket connection errors.
pub fn save_infrastructure_failed_history(
    state: &AppState,
    audio_path: Option<String>,
    error: &str,
) {
    save_failed_history_with_duration_gate(state, audio_path, error, false);
}

fn save_failed_history_with_duration_gate(
    state: &AppState,
    audio_path: Option<String>,
    error: &str,
    enforce_min_duration: bool,
) {
    let recording_duration_ms = {
        let start = state
            .recording_start_time
            .load(std::sync::atomic::Ordering::SeqCst);
        if start > 0 {
            Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64
                    - start as i64,
            )
        } else {
            None
        }
    };

    // Skip saving if recording was too short (< 500ms)
    // This prevents noise from accidental brief recordings
    if should_skip_failed_history_for_duration(recording_duration_ms, enforce_min_duration) {
        let audio_retention = state.settings.lock().audio_retention;
        let store = state.history_store.lock();
        if let Some(path) = apply_audio_retention(&store, audio_path, audio_retention) {
            if let Err(error) =
                store.register_audio_asset(&path, chrono::Utc::now().timestamp_millis())
            {
                tracing::warn!(error = %error, path = %path, "short_recording_audio_registration_failed");
            }
        }
        tracing::info!(
            duration_ms = recording_duration_ms,
            "recording_to_short-skipping_history_save"
        );
        return;
    }

    let (stt_engine, stt_model, language, is_cloud, text_retention, audio_retention) = {
        let settings = state.settings.lock();
        let cloud_config = settings.get_active_cloud_stt_config();
        let is_cloud = cloud_config.enabled;
        let engine_str = if is_cloud {
            format!("cloud-{}", cloud_config.provider_type)
        } else {
            crate::stt_engine::UnifiedEngineManager::get_engine_by_model_name(&settings.model)
                .map(|et| et.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        };
        (
            engine_str,
            if is_cloud {
                Some(cloud_config.model.clone())
            } else {
                Some(settings.model.clone())
            },
            if settings.stt_engine_language.is_empty() {
                None
            } else {
                Some(settings.stt_engine_language.clone())
            },
            is_cloud,
            settings.text_retention,
            settings.audio_retention,
        )
    };

    let store = state.history_store.lock();
    let retained_audio_path = apply_audio_retention(&store, audio_path, audio_retention);
    if !text_retention.retains_new_data() {
        if let Some(path) = retained_audio_path {
            if let Err(save_error) =
                store.register_audio_asset(&path, chrono::Utc::now().timestamp_millis())
            {
                tracing::warn!(error = %save_error, path = %path, "failed_audio_only_asset_registration_failed");
            }
        }
        return;
    }

    let entry = NewTranscriptionEntry {
        raw_text: String::new(),
        final_text: String::new(),
        stt_engine,
        stt_model,
        language,
        audio_duration_ms: recording_duration_ms,
        stt_duration_ms: None,
        polish_duration_ms: None,
        total_duration_ms: None,
        polish_applied: false,
        polish_engine: None,
        is_cloud,
        audio_path: retained_audio_path,
        status: "error".to_string(),
        error: Some(error.to_string()),
        source_kind: "recording".to_string(),
        source_path: None,
        translation_target: None,
        timed_segments: Vec::new(),
        delivery_status: "not_recorded".to_string(),
    };

    if let Err(e) = save_history_entry(&store, entry) {
        tracing::warn!(error = %e, "failed_to_save_failed_history");
    }
}

fn apply_audio_retention(
    store: &HistoryStore,
    audio_path: Option<String>,
    policy: crate::history::RetentionPolicy,
) -> Option<String> {
    let path = audio_path?;
    if policy.retains_new_data() {
        return Some(path);
    }

    match std::fs::remove_file(&path) {
        Ok(()) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            tracing::warn!(error = %error, path = %path, "audio_policy_deletion_failed-tracking_for_retry");
            if let Err(register_error) =
                store.register_audio_asset(&path, chrono::Utc::now().timestamp_millis())
            {
                tracing::error!(error = %register_error, path = %path, "audio_deletion_retry_registration_failed");
            }
            Some(path)
        }
    }
}

fn should_skip_failed_history_for_duration(
    recording_duration_ms: Option<i64>,
    enforce_min_duration: bool,
) -> bool {
    enforce_min_duration
        && recording_duration_ms
            .map(|duration| duration < MIN_FAILED_RECORDING_DURATION_MS)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        paste_last_transcription_with, prune_terminal_file_jobs, resolve_workbench_template_prompt,
        should_skip_failed_history_for_duration, FileJobRecord, FileJobState,
        MAX_TERMINAL_FILE_JOBS,
    };
    use crate::history::{HistoryStore, NewTranscriptionEntry};
    use crate::services::transcription_workbench::FileTranscriptionRequest;
    use crate::text_injector::InjectionMethod;
    use std::cell::Cell;
    use std::path::PathBuf;

    fn successful_history_entry(text: &str) -> NewTranscriptionEntry {
        NewTranscriptionEntry {
            raw_text: format!("raw {text}"),
            final_text: text.to_string(),
            stt_engine: "whisper".to_string(),
            stt_model: None,
            language: None,
            audio_duration_ms: None,
            stt_duration_ms: None,
            polish_duration_ms: None,
            total_duration_ms: None,
            polish_applied: false,
            polish_engine: None,
            is_cloud: false,
            audio_path: None,
            status: "success".to_string(),
            error: None,
            source_kind: "recording".to_string(),
            source_path: None,
            translation_target: None,
            timed_segments: Vec::new(),
            delivery_status: "not_delivered".to_string(),
        }
    }

    #[tokio::test]
    async fn history_polish_preserves_source_but_explicit_translation_can_change_language() {
        use wiremock::{
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{"message": {"content": "Please fix the error and check the output"}}]
            })))
            .expect(2)
            .mount(&server)
            .await;
        let state = crate::state::app_state::AppState::new();
        {
            let mut settings = state.settings.lock();
            settings.cloud_polish_enabled = true;
            settings.active_cloud_polish_provider = "openai".to_string();
            settings.cloud_polish_configs.insert(
                "openai".to_string(),
                crate::commands::settings::CloudProviderConfig {
                    enabled: true,
                    provider_type: "openai".to_string(),
                    api_key: "test-key".to_string(),
                    base_url: server.uri(),
                    model: "test-model".to_string(),
                    enable_thinking: false,
                },
            );
        }
        let source = "Alors je veux que tu vérifies les données et les règles de sécurité dans cette application";

        let cleanup =
            super::polish_workbench_text(&state, source, Some("fr"), None, Some("filler"), false)
                .await
                .unwrap();
        assert_eq!(cleanup.0, source);
        let translated =
            super::polish_workbench_text(&state, source, Some("fr"), Some("en"), None, false)
                .await
                .unwrap();
        assert_eq!(translated.0, "Please fix the error and check the output");
    }

    #[test]
    fn paste_last_transcription_inserts_final_text_and_records_keyboard_delivery() {
        let store = HistoryStore::new_in_memory().unwrap();
        let entry_id = store
            .insert(successful_history_entry("polished final text"))
            .unwrap();
        let mut inserted_text = String::new();

        let status = paste_last_transcription_with(&store, |text| {
            inserted_text = text.to_string();
            Ok(InjectionMethod::Keyboard)
        })
        .unwrap();

        assert_eq!(inserted_text, "polished final text");
        assert_eq!(status, "inserted_keyboard");
        assert_eq!(
            store.get_entry(&entry_id).unwrap().unwrap().delivery_status,
            "inserted_keyboard"
        );
    }

    #[test]
    fn paste_last_transcription_records_clipboard_delivery() {
        let store = HistoryStore::new_in_memory().unwrap();
        let entry_id = store
            .insert(successful_history_entry("multiline\nfinal text"))
            .unwrap();

        let status =
            paste_last_transcription_with(&store, |_| Ok(InjectionMethod::Clipboard)).unwrap();

        assert_eq!(status, "inserted_clipboard");
        assert_eq!(
            store.get_entry(&entry_id).unwrap().unwrap().delivery_status,
            "inserted_clipboard"
        );
    }

    #[test]
    fn paste_last_transcription_records_failure_and_returns_injector_error() {
        let store = HistoryStore::new_in_memory().unwrap();
        let entry_id = store
            .insert(successful_history_entry("recover me"))
            .unwrap();

        let error = paste_last_transcription_with(&store, |_| {
            Err("focused target rejected insertion".to_string())
        })
        .unwrap_err();

        assert_eq!(error, "focused target rejected insertion");
        assert_eq!(
            store.get_entry(&entry_id).unwrap().unwrap().delivery_status,
            "failed"
        );
    }

    #[test]
    fn paste_last_transcription_reports_empty_history_without_calling_injector() {
        let store = HistoryStore::new_in_memory().unwrap();
        let injector_called = Cell::new(false);

        let error = paste_last_transcription_with(&store, |_| {
            injector_called.set(true);
            Ok(InjectionMethod::Keyboard)
        })
        .unwrap_err();

        assert_eq!(error, "No successful transcription is available");
        assert!(!injector_called.get());
    }

    #[test]
    fn failed_history_duration_gate_skips_only_user_recording_failures() {
        assert!(should_skip_failed_history_for_duration(Some(120), true));
        assert!(!should_skip_failed_history_for_duration(Some(120), false));
        assert!(!should_skip_failed_history_for_duration(Some(800), true));
        assert!(!should_skip_failed_history_for_duration(None, true));
    }

    #[test]
    fn explicit_unknown_polish_template_fails_without_fallback() {
        let settings = crate::commands::settings::AppSettings::default();

        assert_eq!(
            resolve_workbench_template_prompt(&settings, Some("missing-template")).unwrap_err(),
            "Polish template not found: missing-template"
        );
        assert!(resolve_workbench_template_prompt(&settings, None)
            .unwrap()
            .contains("Clean raw dictation"));
    }

    #[test]
    fn file_job_cancel_is_backend_owned_and_terminal() {
        let record = FileJobRecord::new(FileTranscriptionRequest {
            path: PathBuf::from("/tmp/source.wav"),
            profile_id: None,
            translation_target: None,
        });
        let running = record.update(FileJobState::Running, 25);
        assert_eq!(running.state, FileJobState::Running);
        assert_eq!(running.progress_percent, 25);

        let canceled = record.cancel().unwrap();
        assert_eq!(canceled.state, FileJobState::Canceled);
        assert!(record.canceled.load(std::sync::atomic::Ordering::SeqCst));
        assert!(record.cancel().unwrap_err().contains("already terminal"));
    }

    #[test]
    fn file_job_pruning_keeps_active_jobs_and_only_the_newest_terminal_jobs() {
        let request = || FileTranscriptionRequest {
            path: PathBuf::from("/tmp/source.wav"),
            profile_id: None,
            translation_target: None,
        };
        let active = std::sync::Arc::new(FileJobRecord::new(request()));
        let active_id = active.snapshot().id;
        let mut jobs = std::collections::HashMap::from([(active_id.clone(), active)]);
        let mut terminal_ids = Vec::new();
        for _ in 0..(MAX_TERMINAL_FILE_JOBS + 2) {
            let record = std::sync::Arc::new(FileJobRecord::new(request()));
            record.update(FileJobState::Completed, 100);
            let id = record.snapshot().id;
            terminal_ids.push(id.clone());
            jobs.insert(id, record);
        }

        prune_terminal_file_jobs(&mut jobs, MAX_TERMINAL_FILE_JOBS);

        assert_eq!(jobs.len(), MAX_TERMINAL_FILE_JOBS + 1);
        assert!(jobs.contains_key(&active_id));
        assert!(!jobs.contains_key(&terminal_ids[0]));
        assert!(!jobs.contains_key(&terminal_ids[1]));
        assert!(jobs.contains_key(terminal_ids.last().unwrap()));
    }
}
