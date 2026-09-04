use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info, warn};

use crate::events::{
    emit_pill_tooltip, emit_recording_state, emit_retry_complete, emit_retry_error,
    emit_retry_state, EventName, RecordingStatus, RetryStatus,
};
use crate::services::transcription_finalize::FinalizeResult;
use crate::state::app_state::AppState;

use super::postprocess::strip_trailing_sentence_period;

pub(crate) type ParkingMutex<T> = parking_lot::Mutex<T>;

pub(crate) const AUDIO_ACTIVITY_ON_THRESHOLD: u32 = 40;
pub(crate) const AUDIO_ACTIVITY_OFF_THRESHOLD: u32 = 25;
pub(crate) const RECORDING_CHUNK_DURATION_MS: usize = 200;
pub(crate) const ERROR_STATE_SETTLE_MS: u64 = 2000;
pub(crate) const PROCESSING_ERROR_TOOLTIP_DURATION_MS: u64 = 4000;
pub(crate) const TRANSCRIPTION_ERROR_TOOLTIP_MESSAGE: &str =
    "Transcription failed. Please try again.";
pub(crate) const POLISH_ERROR_TOOLTIP_MESSAGE: &str =
    "Polish failed. Using original transcription.";
pub(crate) const POLISH_ERROR_TOOLTIP_SUFFIX: &str = "Using original transcription.";
pub(crate) const LOCAL_POLISH_TIMEOUT_TOOLTIP_MESSAGE: &str =
    "Local polish timed out. Using original transcription.";
pub(crate) const POLISH_POLICY_TOOLTIP_MESSAGE: &str =
    "Polish result was rejected. Using original transcription.";
pub(crate) const POLISH_PREVIEW_PENDING_TOOLTIP_MESSAGE: &str = "Polishing...";
pub(crate) const POLISH_PREVIEW_TOOLTIP_DURATION_MS: u64 = 1600;
pub(crate) const TEXT_INJECTION_ERROR_TOOLTIP_MESSAGE: &str =
    "Text delivery failed. Copy the result from history.";
const POLISH_PREVIEW_MAX_CHARS: usize = 220;
const THINK_START_TAG: &str = "<think>";
const THINK_END_TAG: &str = "</think>";

#[derive(Clone)]
pub(crate) struct PolishPreviewHandle {
    pub callback: crate::polish_engine::PolishPreviewCallback,
    direct_stream_inserted: Arc<AtomicBool>,
}

impl PolishPreviewHandle {
    pub(crate) fn direct_stream_inserted(&self) -> bool {
        self.direct_stream_inserted.load(Ordering::SeqCst)
    }
}

pub(crate) fn polish_error_tooltip_message(reason: Option<&str>) -> String {
    match reason.map(str::trim).filter(|reason| !reason.is_empty()) {
        Some(reason) => format!("Polish failed: {reason}. {POLISH_ERROR_TOOLTIP_SUFFIX}"),
        None => POLISH_ERROR_TOOLTIP_MESSAGE.to_string(),
    }
}

pub(crate) fn should_emit_error_recovery_idle(
    current_task_id: u64,
    error_task_id: u64,
    is_recording: bool,
    is_transcribing: bool,
) -> bool {
    current_task_id == error_task_id && !is_recording && !is_transcribing
}

pub(crate) async fn emit_recording_error_then_idle(app: &AppHandle, task_id: u64) {
    emit_recording_state(app, RecordingStatus::Error, task_id);
    emit_pill_tooltip(
        app,
        TRANSCRIPTION_ERROR_TOOLTIP_MESSAGE,
        PROCESSING_ERROR_TOOLTIP_DURATION_MS,
        Some(task_id),
    );
    tokio::time::sleep(tokio::time::Duration::from_millis(ERROR_STATE_SETTLE_MS)).await;

    let state = app.state::<AppState>();
    let current_task_id = state.task_counter.load(Ordering::SeqCst);
    let is_recording = state.is_recording.load(Ordering::SeqCst);
    let is_transcribing = state.is_transcribing.load(Ordering::SeqCst);
    if !should_emit_error_recovery_idle(current_task_id, task_id, is_recording, is_transcribing) {
        info!(
            task_id,
            current_task_id,
            is_recording,
            is_transcribing,
            "error_recovery_idle_skipped_for_stale_task"
        );
        return;
    }

    emit_recording_state(app, RecordingStatus::Idle, task_id);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingState {
    pub is_recording: bool,
    pub is_transcribing: bool,
    pub audio_level: u32,
    pub output_path: Option<String>,
}

pub(crate) async fn apply_finalize_result(
    app: &AppHandle,
    task_id: u64,
    result: FinalizeResult,
    original_target_enabled: bool,
    original_target_mode: crate::commands::settings::OriginalTargetMode,
    original_target: Option<&crate::text_injector::CapturedTextTarget>,
) {
    match result {
        FinalizeResult::DeliverText {
            text,
            history_entry_id,
        } => {
            emit_recording_complete(app, task_id, &text);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let injection_started = Instant::now();
            let insertion = insert_text_with_metrics(
                &text,
                task_id,
                "recording",
                history_entry_id.as_deref(),
                |text| {
                    deliver_recording_text(
                        original_target_enabled,
                        original_target_mode,
                        original_target,
                        text,
                    )
                },
            );
            let inserted_with_accessibility = matches!(
                &insertion,
                Ok((_, crate::text_injector::InjectionMethod::Accessibility))
            );
            update_history_delivery_status(
                app,
                history_entry_id.as_deref(),
                injection_delivery_status(&insertion),
            );
            if let Err(error) = insertion {
                error!(task_id, error = %error, "text_injection_failed");
                record_injection_failure(
                    app,
                    u64::try_from(injection_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                );
                emit_pill_tooltip(
                    app,
                    TEXT_INJECTION_ERROR_TOOLTIP_MESSAGE,
                    PROCESSING_ERROR_TOOLTIP_DURATION_MS,
                    Some(task_id),
                );
                if let Some(runtime) =
                    app.try_state::<crate::services::product_workflows::WorkflowRuntime>()
                {
                    runtime.discard_staged_delivery(task_id);
                    runtime.clear_active_task(task_id);
                }
                return;
            }
            if let Some(runtime) =
                app.try_state::<crate::services::product_workflows::WorkflowRuntime>()
            {
                runtime.commit_staged_delivery(task_id);
                runtime.clear_active_task(task_id);
            }
            let correction_memory_enabled = {
                let state = app.state::<AppState>();
                let settings = state.settings.lock();
                settings.correction_memory_enabled
            };
            if correction_memory_enabled && !inserted_with_accessibility {
                crate::correction_learning::observe_post_delivery_edit(app.clone(), text);
            }
        }
        FinalizeResult::TextAlreadyInserted {
            text,
            history_entry_id,
            disposition,
        } => {
            emit_recording_complete(app, task_id, &text);
            info!(
                task_id,
                history_entry_id = history_entry_id.as_deref().unwrap_or(""),
                ?disposition,
                text_len = text.len(),
                "text_injection_skipped-for_delivery_disposition"
            );
            if let Some(runtime) =
                app.try_state::<crate::services::product_workflows::WorkflowRuntime>()
            {
                if should_observe_post_delivery(disposition) {
                    runtime.commit_staged_delivery(task_id);
                } else {
                    runtime.discard_staged_delivery(task_id);
                }
                runtime.clear_active_task(task_id);
            }
            let correction_memory_enabled = should_observe_post_delivery(disposition) && {
                let state = app.state::<AppState>();
                let settings = state.settings.lock();
                settings.correction_memory_enabled
            };
            if correction_memory_enabled {
                crate::correction_learning::observe_post_delivery_edit(app.clone(), text);
            }
        }
        FinalizeResult::TransitionToIdle => {
            if let Some(runtime) =
                app.try_state::<crate::services::product_workflows::WorkflowRuntime>()
            {
                runtime.discard_staged_delivery(task_id);
                runtime.clear_active_task(task_id);
            }
            emit_recording_state(app, RecordingStatus::Idle, task_id);
        }
        FinalizeResult::TransitionToErrorThenIdle => {
            if let Some(runtime) =
                app.try_state::<crate::services::product_workflows::WorkflowRuntime>()
            {
                runtime.discard_staged_delivery(task_id);
                runtime.clear_active_task(task_id);
            }
            emit_recording_error_then_idle(app, task_id).await;
        }
    }
}

fn deliver_recording_text(
    original_target_enabled: bool,
    mode: crate::commands::settings::OriginalTargetMode,
    target: Option<&crate::text_injector::CapturedTextTarget>,
    text: &str,
) -> Result<crate::text_injector::InjectionMethod, String> {
    let application_id = target.map(crate::text_injector::CapturedTextTarget::application_id);
    deliver_recording_text_with(
        original_target_enabled,
        mode,
        application_id,
        text,
        activate_original_application,
        crate::text_injector::insert_text,
        |text| {
            target
                .ok_or_else(|| "Original editable field was not captured".to_string())?
                .insert_background(text)
        },
    )
}

fn activate_original_application(application_id: &str) -> Result<(), String> {
    crate::sensors::focused_context::activate_application(application_id)?;
    const ATTEMPTS: usize = 12;
    for _ in 0..ATTEMPTS {
        if crate::sensors::focused_context::capture_application_context()
            .application_id
            .as_deref()
            == Some(application_id)
        {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    Err(format!(
        "Original application did not become active: {application_id}"
    ))
}

fn deliver_recording_text_with<Activate, InsertCurrent, InsertBackground>(
    original_target_enabled: bool,
    mode: crate::commands::settings::OriginalTargetMode,
    original_application_id: Option<&str>,
    text: &str,
    activate_original_application: Activate,
    insert_into_current_focus: InsertCurrent,
    insert_into_background_target: InsertBackground,
) -> Result<crate::text_injector::InjectionMethod, String>
where
    Activate: FnOnce(&str) -> Result<(), String>,
    InsertCurrent: FnOnce(&str) -> Result<crate::text_injector::InjectionMethod, String>,
    InsertBackground: FnOnce(&str) -> Result<crate::text_injector::InjectionMethod, String>,
{
    if !original_target_enabled {
        return insert_into_current_focus(text);
    }

    let application_id = original_application_id
        .filter(|application_id| !application_id.trim().is_empty())
        .ok_or_else(|| "Original application was not captured".to_string())?;

    match mode {
        crate::commands::settings::OriginalTargetMode::Foreground => {
            activate_original_application(application_id)?;
            insert_into_current_focus(text)
        }
        crate::commands::settings::OriginalTargetMode::Background => {
            insert_into_background_target(text)
        }
    }
}

fn should_observe_post_delivery(
    disposition: crate::services::transcription_finalize::DeliveryDisposition,
) -> bool {
    disposition
        == crate::services::transcription_finalize::DeliveryDisposition::DirectStreamInserted
}

fn emit_recording_complete(app: &AppHandle, task_id: u64, text: &str) {
    let _ = app.emit(
        EventName::TRANSCRIPTION_COMPLETE,
        crate::events::TranscriptionCompleteEvent {
            text: text.to_string(),
            task_id,
        },
    );
    emit_recording_state(app, RecordingStatus::Idle, task_id);
}

pub(crate) async fn apply_retry_success(app: &AppHandle, entry_id: &str, task_id: u64, text: &str) {
    emit_retry_complete(app, entry_id, task_id, text);
    emit_retry_state(app, entry_id, RetryStatus::Completed, task_id);
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let injection_started = Instant::now();
    if let Err(error) = insert_text_with_metrics(
        text,
        task_id,
        "retry",
        Some(entry_id),
        crate::text_injector::insert_text,
    ) {
        error!(task_id, entry_id, error = %error, "text_injection_failed");
        record_injection_failure(
            app,
            u64::try_from(injection_started.elapsed().as_millis()).unwrap_or(u64::MAX),
        );
        emit_pill_tooltip(
            app,
            TEXT_INJECTION_ERROR_TOOLTIP_MESSAGE,
            PROCESSING_ERROR_TOOLTIP_DURATION_MS,
            None,
        );
        return;
    }
    let correction_memory_enabled = {
        let state = app.state::<AppState>();
        let settings = state.settings.lock();
        settings.correction_memory_enabled
    };
    if correction_memory_enabled {
        crate::correction_learning::observe_post_delivery_edit(app.clone(), text.to_string());
    }
}

fn insert_text_with_metrics<F>(
    text: &str,
    task_id: u64,
    context: &'static str,
    entry_id: Option<&str>,
    insert: F,
) -> Result<(u64, crate::text_injector::InjectionMethod), String>
where
    F: FnOnce(&str) -> Result<crate::text_injector::InjectionMethod, String>,
{
    let injection_started = Instant::now();
    let result = insert(text);
    let injection_ms = injection_started.elapsed().as_millis() as u64;
    match result {
        Ok(method) => {
            info!(
                task_id,
                context,
                entry_id = entry_id.unwrap_or(""),
                text_len = text.len(),
                injection_ms,
                ?method,
                "text_injection_completed"
            );
            Ok((injection_ms, method))
        }
        Err(error) => {
            warn!(
                task_id,
                context,
                entry_id = entry_id.unwrap_or(""),
                text_len = text.len(),
                injection_ms,
                error = %error,
                "text_injection_failed"
            );
            Err(error)
        }
    }
}

fn injection_delivery_status(
    result: &Result<(u64, crate::text_injector::InjectionMethod), String>,
) -> &'static str {
    match result {
        Ok((_, crate::text_injector::InjectionMethod::Keyboard)) => "inserted_keyboard",
        Ok((_, crate::text_injector::InjectionMethod::Clipboard)) => "inserted_clipboard",
        Ok((_, crate::text_injector::InjectionMethod::Accessibility)) => "inserted_accessibility",
        Err(_) => "failed",
    }
}

fn update_history_delivery_status(app: &AppHandle, entry_id: Option<&str>, status: &str) {
    let Some(entry_id) = entry_id else {
        return;
    };
    let state = app.state::<AppState>();
    if let Err(error) = state
        .history_store
        .lock()
        .update_delivery_status(entry_id, status)
    {
        warn!(error = %error, entry_id, status, "history_delivery_status_update_failed");
    };
}

fn record_injection_failure(app: &AppHandle, injection_ms: u64) {
    let application_id = app
        .try_state::<crate::services::product_workflows::WorkflowRuntime>()
        .and_then(|runtime| runtime.context())
        .and_then(|context| context.application_id);
    crate::commands::platform_quality::record_quality_event(
        &crate::services::platform_quality::QualityEvent::injection_failure(
            application_id.as_deref(),
            injection_ms,
        ),
    );
}

pub(crate) fn apply_retry_error(app: &AppHandle, entry_id: &str, task_id: u64, error: &str) {
    emit_retry_error(app, entry_id, task_id, error);
    emit_retry_state(app, entry_id, RetryStatus::Error, task_id);
    emit_pill_tooltip(
        app,
        TRANSCRIPTION_ERROR_TOOLTIP_MESSAGE,
        PROCESSING_ERROR_TOOLTIP_DURATION_MS,
        None,
    );
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub(crate) enum ProcessingEventTarget<'a> {
    None,
    Recording(&'a AppHandle),
    Retry {
        app: &'a AppHandle,
        entry_id: &'a str,
    },
}

impl ProcessingEventTarget<'_> {
    pub(crate) fn emit_polishing(&self, task_id: u64) {
        match self {
            Self::None => {}
            Self::Recording(app) => emit_recording_state(app, RecordingStatus::Polishing, task_id),
            Self::Retry { app, entry_id } => {
                emit_retry_state(app, entry_id, RetryStatus::Polishing, task_id);
            }
        }
    }

    pub(crate) fn emit_polish_error_tooltip(&self, task_id: u64, reason: Option<&str>) {
        self.emit_processing_tooltip(task_id, polish_error_tooltip_message(reason));
    }

    pub(crate) fn emit_local_polish_timeout_tooltip(&self, task_id: u64) {
        self.emit_processing_tooltip(task_id, LOCAL_POLISH_TIMEOUT_TOOLTIP_MESSAGE.to_string());
    }

    pub(crate) fn emit_polish_policy_tooltip(&self, task_id: u64) {
        self.emit_processing_tooltip(task_id, POLISH_POLICY_TOOLTIP_MESSAGE.to_string());
    }

    pub(crate) fn polish_preview_callback(&self, task_id: u64) -> Option<PolishPreviewHandle> {
        let app = match self {
            Self::None => return None,
            Self::Recording(app) => (*app).clone(),
            Self::Retry { app, .. } => (*app).clone(),
        };
        let tooltip_task_id = match self {
            Self::Recording(_) => Some(task_id),
            Self::Retry { .. } => None,
            Self::None => None,
        };
        let original_target_enabled = match self {
            Self::Recording(app) => {
                let state = app.state::<AppState>();
                state.session_uses_original_target(task_id)
            }
            Self::Retry { .. } | Self::None => false,
        };
        let typed_visible_chars = Arc::new(ParkingMutex::new(0_usize));
        let direct_stream_inserted = Arc::new(AtomicBool::new(false));
        let callback_inserted = Arc::clone(&direct_stream_inserted);

        let callback = Arc::new(move |update: crate::polish_engine::PolishPreviewUpdate| {
            if should_type_direct_stream_delta(false, update.is_final, original_target_enabled) {
                maybe_insert_direct_stream_delta(
                    &app,
                    task_id,
                    &update.text,
                    &typed_visible_chars,
                    &callback_inserted,
                );
            }

            let Some(message) = polish_preview_tooltip_message(&update.text) else {
                return;
            };
            emit_pill_tooltip(
                &app,
                message,
                POLISH_PREVIEW_TOOLTIP_DURATION_MS,
                tooltip_task_id,
            );
        });

        Some(PolishPreviewHandle {
            callback,
            direct_stream_inserted,
        })
    }

    fn emit_processing_tooltip(&self, task_id: u64, message: String) {
        match self {
            Self::None => {}
            Self::Recording(app) => emit_pill_tooltip(
                app,
                message,
                PROCESSING_ERROR_TOOLTIP_DURATION_MS,
                Some(task_id),
            ),
            Self::Retry { app, .. } => {
                emit_pill_tooltip(app, message, PROCESSING_ERROR_TOOLTIP_DURATION_MS, None)
            }
        }
    }
}

fn maybe_insert_direct_stream_delta(
    app: &AppHandle,
    task_id: u64,
    accumulated_text: &str,
    typed_visible_chars: &Arc<ParkingMutex<usize>>,
    direct_stream_inserted: &Arc<AtomicBool>,
) {
    let state = app.state::<AppState>();
    if state.task_counter.load(Ordering::SeqCst) != task_id
        || state.is_cancellation_requested(task_id)
    {
        return;
    }

    let mut typed_chars = typed_visible_chars.lock();
    let Some(delta) = next_direct_stream_delta(accumulated_text, &mut typed_chars) else {
        return;
    };
    drop(typed_chars);

    let injection_started = Instant::now();
    let injection_result = insert_text_with_metrics(
        &delta,
        task_id,
        "recording-polish-stream",
        None,
        crate::text_injector::insert_text,
    );
    match injection_result {
        Ok(_) => direct_stream_inserted.store(true, Ordering::SeqCst),
        Err(_) => record_injection_failure(
            app,
            u64::try_from(injection_started.elapsed().as_millis()).unwrap_or(u64::MAX),
        ),
    }
}

fn should_type_direct_stream_delta(
    direct_typing_enabled: bool,
    is_final_update: bool,
    original_target_enabled: bool,
) -> bool {
    direct_typing_enabled && !is_final_update && !original_target_enabled
}

fn next_direct_stream_delta(
    accumulated_text: &str,
    typed_visible_chars: &mut usize,
) -> Option<String> {
    let raw_visible_text = sanitize_polish_preview_text(accumulated_text)?;
    let (visible_text, _) = strip_trailing_sentence_period(raw_visible_text);
    let visible_chars = visible_text.chars().count();
    if visible_chars <= *typed_visible_chars {
        return None;
    }

    let delta = visible_text
        .chars()
        .skip(*typed_visible_chars)
        .collect::<String>();
    *typed_visible_chars = visible_chars;

    (!delta.is_empty()).then_some(delta)
}

pub(crate) fn polish_preview_tooltip_message(text: &str) -> Option<String> {
    let raw_text = text.trim();
    if raw_text.is_empty() {
        return None;
    }

    let Some(raw_text) = sanitize_polish_preview_text(raw_text) else {
        if raw_text.contains(THINK_START_TAG) {
            return Some(POLISH_PREVIEW_PENDING_TOOLTIP_MESSAGE.to_string());
        }
        return None;
    };
    let (text, _) = strip_trailing_sentence_period(raw_text);

    let mut preview = text
        .chars()
        .take(POLISH_PREVIEW_MAX_CHARS)
        .collect::<String>();

    if text.chars().count() > POLISH_PREVIEW_MAX_CHARS {
        preview.push_str("...");
    }

    Some(preview)
}

fn sanitize_polish_preview_text(text: &str) -> Option<&str> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    if let Some(end_idx) = text.find(THINK_END_TAG) {
        let text = text[end_idx + THINK_END_TAG.len()..].trim();
        return (!text.is_empty()).then_some(text);
    }

    if text.contains(THINK_START_TAG) {
        return None;
    }

    Some(text)
}

pub(crate) fn await_streaming_task_in_background(
    task_id: u64,
    handle: tauri::async_runtime::JoinHandle<()>,
) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = handle.await {
            error!(task_id, error = %e, "streaming_stt_task_panicked");
        }
    });
}

pub(crate) fn discard_canceled_result(state: &AppState, task_id: u64, audio_path: Option<&String>) {
    tracing::info!(task_id, "recording_canceled_discarding_result");

    if let Some(path) = audio_path {
        if let Err(e) = std::fs::remove_file(path) {
            warn!(task_id, error = %e, path = %path, "canceled_audio_cleanup_failed");
        }
    }

    state.is_transcribing.store(false, Ordering::SeqCst);
    let _ = state.finish_session(task_id);
    state.clear_cancellation(task_id);
}

pub(crate) fn recording_chunk_size_samples(sample_rate: u32, channels: u16) -> usize {
    (sample_rate as usize * channels as usize * RECORDING_CHUNK_DURATION_MS) / 1000
}

pub(crate) fn flush_pending_chunk_for_stop(
    chunk_buffer: &Arc<ParkingMutex<Vec<i16>>>,
    processor: &Arc<ParkingMutex<crate::audio::stream_processor::StreamAudioProcessor>>,
    sample_rate: u32,
    channels: u16,
) -> Option<Vec<i16>> {
    let pending_pcm = {
        let mut buffer = chunk_buffer.lock();
        if buffer.is_empty() || sample_rate == 0 || channels == 0 {
            return None;
        }
        buffer.drain(..).collect::<Vec<i16>>()
    };

    let audio_f32: Vec<f32> = pending_pcm.iter().map(|&s| s as f32 / 32768.0).collect();

    let mono_f32 = if channels == 2 {
        audio_f32
            .chunks(2)
            .map(|stereo| (stereo[0] + stereo.get(1).copied().unwrap_or(0.0)) / 2.0)
            .collect()
    } else {
        audio_f32
    };

    let result = processor
        .lock()
        .process_chunk_for_stop_flush(&mono_f32, sample_rate);
    if result.pcm_16khz_mono.is_empty() {
        None
    } else {
        Some(result.pcm_16khz_mono)
    }
}

pub(crate) async fn send_flushed_chunk_for_stop(
    audio_tx: &tokio::sync::mpsc::Sender<Vec<i16>>,
    chunk: Vec<i16>,
) -> Result<(), tokio::sync::mpsc::error::SendError<Vec<i16>>> {
    audio_tx.send(chunk).await
}

pub(crate) fn should_unregister_cancel_hotkey_after_async_cleanup(
    active_task_id: u64,
    cleanup_task_id: u64,
    cancellation_requested: bool,
) -> bool {
    active_task_id == cleanup_task_id && !cancellation_requested
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::{
        deliver_recording_text_with, injection_delivery_status, insert_text_with_metrics,
        next_direct_stream_delta, polish_error_tooltip_message, polish_preview_tooltip_message,
        should_observe_post_delivery, should_type_direct_stream_delta,
        LOCAL_POLISH_TIMEOUT_TOOLTIP_MESSAGE, POLISH_ERROR_TOOLTIP_MESSAGE,
        POLISH_PREVIEW_MAX_CHARS, POLISH_PREVIEW_PENDING_TOOLTIP_MESSAGE,
    };

    #[test]
    fn polish_error_tooltip_includes_reason_when_available() {
        assert_eq!(
            polish_error_tooltip_message(Some("model load failed")),
            "Polish failed: model load failed. Using original transcription."
        );
    }

    #[test]
    fn polish_error_tooltip_keeps_legacy_message_without_reason() {
        assert_eq!(
            polish_error_tooltip_message(None),
            POLISH_ERROR_TOOLTIP_MESSAGE
        );
        assert_eq!(
            polish_error_tooltip_message(Some("   ")),
            POLISH_ERROR_TOOLTIP_MESSAGE
        );
    }

    #[test]
    fn local_polish_timeout_tooltip_is_specific() {
        assert_eq!(
            LOCAL_POLISH_TIMEOUT_TOOLTIP_MESSAGE,
            "Local polish timed out. Using original transcription."
        );
    }

    #[test]
    fn insert_text_with_metrics_invokes_inserter() {
        let called = Arc::new(AtomicBool::new(false));
        let callback_called = Arc::clone(&called);

        let (injection_ms, method) =
            insert_text_with_metrics("hello", 42, "test", Some("entry-1"), move |text| {
                assert_eq!(text, "hello");
                callback_called.store(true, Ordering::SeqCst);
                Ok(crate::text_injector::InjectionMethod::Keyboard)
            })
            .unwrap();

        assert!(called.load(Ordering::SeqCst));
        assert!(injection_ms < 1_000);
        assert_eq!(method, crate::text_injector::InjectionMethod::Keyboard);
    }

    #[test]
    fn insert_text_with_metrics_propagates_delivery_failure() {
        let error = insert_text_with_metrics("hello", 42, "test", None, |_| {
            Err("target rejected input".to_string())
        })
        .unwrap_err();

        assert_eq!(error, "target rejected input");
    }

    #[test]
    fn injection_result_maps_to_exact_history_delivery_status() {
        assert_eq!(
            injection_delivery_status(&Ok((1, crate::text_injector::InjectionMethod::Keyboard,))),
            "inserted_keyboard"
        );
        assert_eq!(
            injection_delivery_status(&Ok((1, crate::text_injector::InjectionMethod::Clipboard,))),
            "inserted_clipboard"
        );
        assert_eq!(
            injection_delivery_status(&Ok((
                1,
                crate::text_injector::InjectionMethod::Accessibility,
            ))),
            "inserted_accessibility"
        );
        assert_eq!(
            injection_delivery_status(&Err("target rejected input".to_string())),
            "failed"
        );
    }

    #[test]
    fn post_delivery_observation_only_runs_for_text_inserted_by_the_stream() {
        use crate::services::transcription_finalize::DeliveryDisposition;

        assert!(should_observe_post_delivery(
            DeliveryDisposition::DirectStreamInserted
        ));
        for disposition in [
            DeliveryDisposition::Preview,
            DeliveryDisposition::Copied,
            DeliveryDisposition::CopyFailed,
        ] {
            assert!(!should_observe_post_delivery(disposition));
        }
    }

    #[test]
    fn direct_stream_typing_requires_explicit_non_final_update() {
        assert!(should_type_direct_stream_delta(true, false, false));
        assert!(!should_type_direct_stream_delta(false, false, false));
        assert!(!should_type_direct_stream_delta(true, true, false));
        assert!(!should_type_direct_stream_delta(true, false, true));
    }

    #[test]
    fn disabled_original_target_uses_current_focus_only() {
        let result = deliver_recording_text_with(
            false,
            crate::commands::settings::OriginalTargetMode::Foreground,
            Some("com.example.Editor"),
            "hello",
            |_| panic!("disabled delivery must not activate an application"),
            |_| Ok(crate::text_injector::InjectionMethod::Keyboard),
            |_| panic!("disabled delivery must not use accessibility"),
        )
        .unwrap();

        assert_eq!(result, crate::text_injector::InjectionMethod::Keyboard);
    }

    #[test]
    fn foreground_original_target_activates_before_current_focus_injection() {
        let calls = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let activation_calls = Arc::clone(&calls);
        let insertion_calls = Arc::clone(&calls);

        let result = deliver_recording_text_with(
            true,
            crate::commands::settings::OriginalTargetMode::Foreground,
            Some("com.example.Editor"),
            "hello",
            move |application_id| {
                activation_calls
                    .lock()
                    .push(format!("activate:{application_id}"));
                Ok(())
            },
            move |_| {
                insertion_calls.lock().push("insert".to_string());
                Ok(crate::text_injector::InjectionMethod::Clipboard)
            },
            |_| panic!("foreground delivery must not use accessibility"),
        )
        .unwrap();

        assert_eq!(result, crate::text_injector::InjectionMethod::Clipboard);
        assert_eq!(
            calls.lock().as_slice(),
            ["activate:com.example.Editor", "insert"]
        );
    }

    #[test]
    fn background_original_target_never_activates_or_uses_current_focus() {
        let result = deliver_recording_text_with(
            true,
            crate::commands::settings::OriginalTargetMode::Background,
            Some("com.example.Editor"),
            "hello",
            |_| panic!("background delivery must not activate an application"),
            |_| panic!("background delivery must not inject into current focus"),
            |text| {
                assert_eq!(text, "hello");
                Ok(crate::text_injector::InjectionMethod::Accessibility)
            },
        )
        .unwrap();

        assert_eq!(result, crate::text_injector::InjectionMethod::Accessibility);
    }

    #[test]
    fn targeted_delivery_rejects_a_missing_original_application() {
        let error = deliver_recording_text_with(
            true,
            crate::commands::settings::OriginalTargetMode::Background,
            None,
            "hello",
            |_| panic!("missing target must not activate an application"),
            |_| panic!("missing target must not inject into current focus"),
            |_| panic!("missing target must not invoke accessibility"),
        )
        .unwrap_err();

        assert_eq!(error, "Original application was not captured");
    }

    #[test]
    fn direct_stream_delta_returns_only_new_visible_text() {
        let mut typed_chars = 0;

        assert_eq!(
            next_direct_stream_delta("Hello", &mut typed_chars).as_deref(),
            Some("Hello")
        );
        assert_eq!(typed_chars, 5);
        assert_eq!(
            next_direct_stream_delta("Hello world", &mut typed_chars).as_deref(),
            Some(" world")
        );
        assert_eq!(typed_chars, 11);
        assert_eq!(
            next_direct_stream_delta("Hello world", &mut typed_chars),
            None
        );
    }

    #[test]
    fn direct_stream_delta_holds_trailing_sentence_period_until_more_text_arrives() {
        let mut typed_chars = 0;

        assert_eq!(
            next_direct_stream_delta("Hello.", &mut typed_chars).as_deref(),
            Some("Hello")
        );
        assert_eq!(typed_chars, 5);

        assert_eq!(next_direct_stream_delta("Hello.", &mut typed_chars), None);
        assert_eq!(typed_chars, 5);

        assert_eq!(
            next_direct_stream_delta("Hello. Again", &mut typed_chars).as_deref(),
            Some(". Again")
        );
        assert_eq!(typed_chars, 12);
    }

    #[test]
    fn direct_stream_delta_hides_incomplete_thinking() {
        let mut typed_chars = 0;

        assert_eq!(
            next_direct_stream_delta("<think>hidden reasoning", &mut typed_chars),
            None
        );
        assert_eq!(typed_chars, 0);
        assert_eq!(
            next_direct_stream_delta("<think>hidden</think>\nVisible", &mut typed_chars).as_deref(),
            Some("Visible")
        );
    }

    #[test]
    fn polish_preview_tooltip_truncates_long_text() {
        let message = polish_preview_tooltip_message(&"a".repeat(300)).unwrap();

        assert!(!message.contains("Polishing preview"));
        assert!(message.ends_with("..."));
        assert_eq!(message.chars().count(), POLISH_PREVIEW_MAX_CHARS + 3);
    }

    #[test]
    fn polish_preview_tooltip_shows_pending_message_for_incomplete_thinking() {
        assert_eq!(
            polish_preview_tooltip_message("<think>hidden reasoning").unwrap(),
            POLISH_PREVIEW_PENDING_TOOLTIP_MESSAGE
        );
        assert_eq!(
            polish_preview_tooltip_message("<think>hidden</think>").unwrap(),
            POLISH_PREVIEW_PENDING_TOOLTIP_MESSAGE
        );
        assert_eq!(
            polish_preview_tooltip_message("<think>hidden</think>\nVisible text").unwrap(),
            "Visible text"
        );
    }
}
