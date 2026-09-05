use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tracing::{debug, error, info, warn};

use crate::commands::settings::CloudSttConfig;
use crate::events::{emit_recording_state, EventName, RecordingStatus, TranscriptionPartialEvent};
use crate::services::transcription_finalize::{
    finalize_empty_transcription, finalize_failed_transcription, finalize_silent_recording,
    finalize_successful_transcription_for_output, DeliveryDisposition,
};
use crate::state::app_state::AppState;
use crate::state::unified_state::StreamingSttState;
use crate::stt_engine::cloud::StreamingSttClient;
use crate::stt_engine::traits::RecordingConsumer;
use crate::utils::AppPaths;

use super::polish::{maybe_polish_transcription_text_for_profile, PolishProcessingResult};
use super::postprocess::apply_post_stt_processing;
use super::shared::{
    apply_finalize_result, discard_canceled_result, emit_recording_error_then_idle,
    recording_chunk_size_samples, should_unregister_cancel_hotkey_after_async_cleanup,
    ParkingMutex, ProcessingEventTarget,
};

const WINDOW_CONTEXT_CAPTURE_TIMEOUT_MS: u64 = 8_000;
const WINDOW_CONTEXT_RECORDING_POLL_MS: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkflowDeliveryPlan {
    Insert,
    Preview,
    Copy,
}

pub(super) struct OriginalTargetSession {
    pub enabled: bool,
    pub mode: crate::commands::settings::OriginalTargetMode,
    pub target: Option<crate::text_injector::CapturedTextTarget>,
    pub application_id: Option<String>,
}

pub(super) fn should_capture_application_context(
    application_metadata_enabled: bool,
    original_target_enabled: bool,
    vibe_coding_enabled: bool,
) -> bool {
    application_metadata_enabled || original_target_enabled || vibe_coding_enabled
}

struct WorkflowTaskCleanup {
    app: AppHandle,
    task_id: u64,
}

impl Drop for WorkflowTaskCleanup {
    fn drop(&mut self) {
        if let Some(runtime) = self
            .app
            .try_state::<crate::services::product_workflows::WorkflowRuntime>()
        {
            runtime.discard_staged_delivery(self.task_id);
            runtime.clear_active_task(self.task_id);
        }
    }
}

pub(super) fn workflow_delivery_plan(
    output_action: Option<crate::services::product_workflows::OutputAction>,
) -> WorkflowDeliveryPlan {
    match output_action.unwrap_or(crate::services::product_workflows::OutputAction::Insert) {
        crate::services::product_workflows::OutputAction::Insert => WorkflowDeliveryPlan::Insert,
        crate::services::product_workflows::OutputAction::Preview => WorkflowDeliveryPlan::Preview,
        crate::services::product_workflows::OutputAction::Copy => WorkflowDeliveryPlan::Copy,
    }
}

fn transcription_quality_event(
    application_id: Option<&str>,
    final_text: &str,
    stt_ms: u64,
    polish_ms: u64,
    is_cloud: bool,
) -> crate::services::platform_quality::QualityEvent {
    if final_text.is_empty() {
        crate::services::platform_quality::QualityEvent::transcription_failure(
            application_id,
            stt_ms.saturating_add(polish_ms),
            is_cloud,
        )
    } else {
        crate::services::platform_quality::QualityEvent::success_with_source(
            application_id,
            stt_ms,
            polish_ms,
            stt_ms.saturating_add(polish_ms),
            is_cloud,
        )
    }
}

pub(super) fn should_cancel_window_context_capture(
    is_current_task: bool,
    is_recording: bool,
    cancellation_requested: bool,
) -> bool {
    !is_current_task || !is_recording || cancellation_requested
}

async fn wait_for_recording_to_end(app: AppHandle, task_id: u64) {
    loop {
        let should_stop_waiting = {
            let state = app.state::<AppState>();
            let is_current_task = state.task_counter.load(Ordering::SeqCst) == task_id;
            should_cancel_window_context_capture(
                is_current_task,
                state.is_recording.load(Ordering::SeqCst),
                state.is_cancellation_requested(task_id),
            )
        };

        if should_stop_waiting {
            return;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(
            WINDOW_CONTEXT_RECORDING_POLL_MS,
        ))
        .await;
    }
}

async fn capture_structured_context_while_recording(
    app: AppHandle,
    task_id: u64,
) -> Option<crate::services::product_workflows::CapturedContext> {
    let context_settings = {
        let state = app.state::<AppState>();
        let context_settings = state.settings.lock().context_capture.clone();
        context_settings
    };
    let clipboard_text = if context_settings.clipboard {
        app.clipboard().read_text().ok()
    } else {
        None
    };
    info!(
        task_id,
        timeout_ms = WINDOW_CONTEXT_CAPTURE_TIMEOUT_MS,
        "structured_context_capture_started"
    );

    let mut context_task = tauri::async_runtime::spawn(async move {
        crate::sensors::focused_context::capture_focused_context(&context_settings, clipboard_text)
            .await
    });

    tokio::select! {
        result = &mut context_task => {
            match result {
                Ok(ctx) => {
                    let state = app.state::<AppState>();
                    if should_cancel_window_context_capture(
                        state.task_counter.load(Ordering::SeqCst) == task_id,
                        state.is_recording.load(Ordering::SeqCst),
                        state.is_cancellation_requested(task_id),
                    ) {
                        info!(task_id, "window_context_capture_discarded-recording_ended");
                        return None;
                    }

                    info!(
                        task_id,
                        sources = ?ctx.sources,
                        selected_chars = ctx.selected_text.as_deref().map(str::len).unwrap_or(0),
                        ocr_chars = ctx.ocr_text.as_deref().map(str::len).unwrap_or(0),
                        has_stt_hint = ctx.to_stt_prompt_hint().is_some(),
                        "structured_context_capture_available"
                    );
                    Some(ctx)
                }
                Err(e) => {
                    warn!(
                        task_id,
                        error = %e,
                        "structured_context_capture_task_failed"
                    );
                    None
                }
            }
        }
        () = tokio::time::sleep(tokio::time::Duration::from_millis(WINDOW_CONTEXT_CAPTURE_TIMEOUT_MS)) => {
            context_task.abort();
            warn!(
                task_id,
                timeout_ms = WINDOW_CONTEXT_CAPTURE_TIMEOUT_MS,
                "window_context_capture_timeout"
            );
            None
        }
        () = wait_for_recording_to_end(app.clone(), task_id) => {
            context_task.abort();
            info!(task_id, "window_context_capture_canceled-recording_ended");
            None
        }
    }
}

pub(super) fn start_unified_recording(
    app: &AppHandle,
    task_id: u64,
    cloud_stt_enabled: bool,
    config: CloudSttConfig,
    language: String,
    resolved_polish_template_id: Option<String>,
    original_target: OriginalTargetSession,
) -> Result<(), String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "AppState not available".to_string())?;
    let audio_device = {
        let settings = state.settings.lock();
        settings.audio_device.clone()
    };

    let (denoise_mode, vad_enabled, domain, subdomain, glossary, retain_audio) = {
        let settings = state.settings.lock();
        let d = settings.stt_engine_work_domain.trim().to_string();
        let s = settings.stt_engine_work_subdomain.trim().to_string();
        let g = settings.stt_engine_user_glossary.trim().to_string();
        (
            settings.denoise_mode.clone(),
            settings.vad_enabled,
            if d.is_empty() { None } else { Some(d) },
            if s.is_empty() { None } else { Some(s) },
            if g.is_empty() { None } else { Some(g) },
            settings.audio_retention.retains_new_data(),
        )
    };

    let (app_tx, mut app_rx) = tokio::sync::mpsc::channel::<Vec<i16>>(100);

    let audio_save_path = retain_audio.then(|| {
        AppPaths::recordings_dir().join(format!(
            "{}_{}.wav",
            chrono::Utc::now().format("%Y%m%d_%H%M%S"),
            task_id
        ))
    });

    if retain_audio {
        if let Err(e) = std::fs::create_dir_all(AppPaths::recordings_dir()) {
            warn!(error = %e, "recordings_directory_creation_failed");
        }
    }

    let raw_audio_buffer: Arc<ParkingMutex<Vec<i16>>> = Arc::new(ParkingMutex::new(Vec::new()));
    let raw_audio_buffer_clone = raw_audio_buffer.clone();

    let device_name = if audio_device == "default" {
        None
    } else {
        Some(audio_device)
    };

    let sample_rate: Arc<parking_lot::Mutex<u32>> = Arc::new(parking_lot::Mutex::new(0));
    let channels: Arc<parking_lot::Mutex<u16>> = Arc::new(parking_lot::Mutex::new(0));
    let chunk_buffer: Arc<parking_lot::Mutex<Vec<i16>>> =
        Arc::new(parking_lot::Mutex::new(Vec::new()));
    let app_tx_clone = app_tx.clone();

    let sample_rate_clone = sample_rate.clone();
    let channels_clone = channels.clone();

    let vad_model_path = state.engine_manager.vad_model_path();
    let vad_model_exists = state.engine_manager.is_vad_model_downloaded();
    let vad_path_arg = if vad_enabled && vad_model_exists {
        Some(vad_model_path.as_path())
    } else {
        None
    };

    let processor: Arc<ParkingMutex<crate::audio::stream_processor::StreamAudioProcessor>> =
        Arc::new(ParkingMutex::new(
            crate::audio::stream_processor::StreamAudioProcessor::new(
                &denoise_mode,
                vad_enabled,
                vad_path_arg,
            ),
        ));

    *state.streaming_stt.lock() = Some(StreamingSttState {
        audio_tx: app_tx.clone(),
        accumulated_text: String::new(),
        task_id,
        streaming_task: Arc::new(ParkingMutex::new(None)),
        audio_save_path,
        raw_audio_buffer: raw_audio_buffer.clone(),
        chunk_buffer: chunk_buffer.clone(),
        processor: processor.clone(),
        sample_rate: 0,
        channels: 0,
    });

    let (sr, ch) = {
        let recorder = state.recorder.lock();
        recorder
            .start_streaming(device_name, move |pcm, sr, ch| {
                if *sample_rate_clone.lock() == 0 {
                    *sample_rate_clone.lock() = sr;
                    *channels_clone.lock() = ch;
                }

                raw_audio_buffer_clone.lock().extend_from_slice(pcm);

                let mut buffer = chunk_buffer.lock();
                buffer.extend_from_slice(pcm);

                let chunk_size = recording_chunk_size_samples(sr, ch);

                if buffer.len() >= chunk_size {
                    let chunk_data = buffer.drain(..).collect::<Vec<i16>>();

                    let audio_f32: Vec<f32> =
                        chunk_data.iter().map(|&s| s as f32 / 32768.0).collect();

                    let mono_f32 = if ch == 2 {
                        audio_f32
                            .chunks(2)
                            .map(|stereo| (stereo[0] + stereo.get(1).copied().unwrap_or(0.0)) / 2.0)
                            .collect()
                    } else {
                        audio_f32
                    };

                    let result = processor.lock().process_chunk(&mono_f32, sr);

                    if result.has_speech {
                        if let Err(e) = app_tx_clone.try_send(result.pcm_16khz_mono) {
                            warn!(task_id, error = %e, "audio_chunk_enqueue_failed-streaming");
                        } else {
                            debug!(task_id, "audio_chunk_enqueued-streaming");
                        }
                    } else {
                        debug!(task_id, "audio_chunk_skipped-silent");
                    }
                }
            })
            .map_err(|e| {
                error!(error = %e, "recorder_start_failed-cloud");
                state.is_recording.store(false, Ordering::SeqCst);
                crate::commands::window::update_pill_visibility(app);
                e.to_string()
            })?
    };

    {
        let mut streaming_stt = state.streaming_stt.lock();
        if let Some(stt) = streaming_stt.as_mut() {
            stt.sample_rate = sr;
            stt.channels = ch;
        }
    }

    let sr_for_async = sr;
    let ch_for_async = ch;

    let app_clone = app.clone();
    let resolved_polish_template_id_clone = resolved_polish_template_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let _workflow_cleanup = WorkflowTaskCleanup {
            app: app_clone.clone(),
            task_id,
        };
        let structured_context =
            capture_structured_context_while_recording(app_clone.clone(), task_id).await;
        let quality_application_id = structured_context
            .as_ref()
            .and_then(|context| context.application_id.clone());
        let vibe_context = {
            let state = app_clone.state::<AppState>();
            let enabled = state.settings.lock().vibe_coding_enabled;
            let active = crate::services::platform_quality::get_active_code_context_snapshot()
                .ok()
                .flatten();
            crate::services::vibe_coding::context_for_recording(
                enabled,
                active.as_ref(),
                original_target.application_id.as_deref(),
                chrono::Utc::now().timestamp_millis(),
            )
        };

        if let (Some(runtime), Some(context)) = (
            app_clone.try_state::<crate::services::product_workflows::WorkflowRuntime>(),
            structured_context.as_ref(),
        ) {
            runtime.set_context(context.clone());
        }

        let window_context = structured_context.as_ref().and_then(|context| {
            let reference = context.to_polish_reference()?;
            match context.ocr_text.as_ref() {
                Some(ocr_text) => {
                    crate::runtime_context::window::WindowContextBundle::from_ocr_result(
                        ocr_text,
                        crate::runtime_context::window::WindowContextSource::FocusedWindow,
                        context.window_title.clone(),
                        0,
                        0,
                        None,
                    )
                    .map(|bundle| bundle.with_structured_reference(reference))
                }
                None => {
                    crate::runtime_context::window::WindowContextBundle::from_structured_reference(
                        reference,
                        context.window_title.clone(),
                    )
                }
            }
        });

        if let Some(ref ctx) = window_context {
            let state_for_session = app_clone.state::<AppState>();
            let mut session = state_for_session.session_state.lock();
            if let Some(s) = session.as_mut() {
                s.window_context = Some(ctx.clone());
            }
        }

        let stt_initial_prompt = {
            let mut hints = Vec::new();
            if let Some(hint) = structured_context
                .as_ref()
                .and_then(|context| context.to_stt_prompt_hint())
            {
                hints.push(hint);
            }
            if let Some(context) = vibe_context.as_ref() {
                hints
                    .push(crate::services::platform_quality::build_code_aware_instruction(context));
            }
            (!hints.is_empty()).then(|| hints.join("\n\n"))
        };

        let stt_context = crate::stt_engine::traits::SttContext {
            domain,
            subdomain,
            glossary,
            initial_prompt: stt_initial_prompt,
        };

        let consumer: Box<dyn RecordingConsumer> = if cloud_stt_enabled {
            let stt_initial_prompt_chars = stt_context
                .initial_prompt
                .as_deref()
                .map(|value| value.chars().count())
                .unwrap_or(0);
            let glossary_chars = stt_context
                .glossary
                .as_deref()
                .map(|value| value.chars().count())
                .unwrap_or(0);
            let (domain, subdomain) = (
                stt_context
                    .domain
                    .clone()
                    .unwrap_or_else(|| "none".to_owned()),
                stt_context
                    .subdomain
                    .clone()
                    .unwrap_or_else(|| "none".to_owned()),
            );

            let client = match StreamingSttClient::new(config, Some(&language), stt_context) {
                Ok(c) => c,
                Err(e) => {
                    error!(task_id, error = %e, "streaming_client_create_failed");
                    crate::commands::platform_quality::record_quality_event(
                        &crate::services::platform_quality::QualityEvent::transcription_failure(
                            quality_application_id.as_deref(),
                            0,
                            true,
                        ),
                    );
                    let state_inner = app_clone.state::<AppState>();
                    crate::history::commands::save_infrastructure_failed_history(
                        &state_inner,
                        None,
                        &e,
                    );
                    emit_recording_error_then_idle(&app_clone, task_id).await;
                    return;
                }
            };
            let provider_name = client.provider_name();
            info!(
                task_id,
                provider = %provider_name,
                domain,
                subdomain,
                has_glossary = glossary_chars > 0,
                glossary_chars,
                has_initial_prompt = stt_initial_prompt_chars > 0,
                initial_prompt_chars = stt_initial_prompt_chars,
                "streaming_client_created"
            );

            let app_event_clone = app_clone.clone();
            let callback = Arc::new(move |result: crate::stt_engine::traits::PartialResult| {
                if !result.is_final && !result.text.is_empty() {
                    let _ = app_event_clone.emit(
                        EventName::TRANSCRIPTION_PARTIAL,
                        TranscriptionPartialEvent {
                            text: result.text,
                            is_definite: result.is_definite,
                            task_id,
                        },
                    );
                }
            });

            match crate::stt_engine::cloud::StreamingConsumer::new(client, callback).await {
                Ok(consumer) => {
                    info!(task_id, provider = %provider_name, "streaming_consumer_connected");
                    Box::new(consumer) as Box<dyn RecordingConsumer>
                }
                Err(e) => {
                    error!(task_id, provider = %provider_name, error = %e, "streaming_consumer_connect_failed");
                    crate::commands::platform_quality::record_quality_event(
                        &crate::services::platform_quality::QualityEvent::transcription_failure(
                            quality_application_id.as_deref(),
                            0,
                            true,
                        ),
                    );
                    let state_inner = app_clone.state::<AppState>();
                    crate::history::commands::save_infrastructure_failed_history(
                        &state_inner,
                        None,
                        &e,
                    );
                    emit_recording_error_then_idle(&app_clone, task_id).await;
                    return;
                }
            }
        } else {
            let state_inner = app_clone.state::<AppState>();
            let (model_name, lang, initial_prompt) = {
                let settings = state_inner.settings.lock();
                (
                    settings.model.clone(),
                    settings.stt_engine_language.clone(),
                    settings.stt_engine_initial_prompt.clone(),
                )
            };

            let (_resolved_engine_type, resolved_model_name) = state_inner
                .engine_manager
                .resolve_available_model(&model_name, &lang);

            if resolved_model_name != model_name {
                info!(
                    requested = %model_name,
                    resolved = %resolved_model_name,
                    "model_fallback_applied"
                );
                let _ = app_clone.emit(
                    EventName::MODEL_RESOLVED,
                    crate::events::ModelResolvedEvent {
                        requested: model_name.clone(),
                        resolved: resolved_model_name.clone(),
                    },
                );
            }

            let engine = crate::stt_engine::buffering_engine::BufferingConsumer::new(
                state_inner.engine_manager.clone(),
                resolved_model_name,
                lang,
                Some(initial_prompt),
                stt_context,
            );

            Box::new(engine) as Box<dyn RecordingConsumer>
        };

        let mut chunks_sent = 0;
        while let Some(chunk) = app_rx.recv().await {
            if let Err(e) = consumer.send_chunk(chunk).await {
                error!(task_id, error = %e, "audio_chunk_send_failed");
                break;
            }
            chunks_sent += 1;
        }
        info!(task_id, total_chunks = chunks_sent, "audio_chunks_all_sent");

        let state_inner = app_clone.state::<AppState>();
        if state_inner.is_cancellation_requested(task_id) {
            discard_canceled_result(&state_inner, task_id, None);
            return;
        }

        if chunks_sent == 0 {
            info!(task_id, "transcription_skipped_no_audio_chunks");
            let action = finalize_silent_recording(None);
            let _ = state_inner.finish_session(task_id);
            apply_finalize_result(
                &app_clone,
                task_id,
                action,
                original_target.enabled,
                original_target.mode,
                original_target.target.as_ref(),
            )
            .await;
        } else {
            emit_recording_state(&app_clone, RecordingStatus::Transcribing, task_id);
            state_inner.is_transcribing.store(true, Ordering::SeqCst);

            debug!(task_id, "consumer_finish_invoked");
            let stt_started = Instant::now();
            let text_result: Result<String, String> = consumer.finish().await;
            let stt_wall_ms = u64::try_from(stt_started.elapsed().as_millis()).unwrap_or(u64::MAX);

            let state_inner = app_clone.state::<AppState>();
            if state_inner.is_cancellation_requested(task_id) {
                discard_canceled_result(&state_inner, task_id, None);
                return;
            }

            let audio_path = {
                let state = app_clone.state::<AppState>();
                let streaming_stt = state.streaming_stt.lock();
                streaming_stt.as_ref().and_then(|s| {
                    crate::audio::wav_writer::save_raw_audio_to_file(s, sr_for_async, ch_for_async)
                })
            };

            match text_result {
                Ok(text) => {
                    let raw_text = text.clone();
                    let (
                        correction_memory_enabled,
                        user_glossary,
                        custom_dictionary,
                        voice_snippets,
                    ) = {
                        let state = app_clone.state::<AppState>();
                        let settings = state.settings.lock();
                        (
                            settings.correction_memory_enabled,
                            settings.stt_engine_user_glossary.clone(),
                            settings.custom_dictionary.clone(),
                            settings.voice_snippets.clone(),
                        )
                    };
                    let snippet_context = structured_context.clone().unwrap_or_default();
                    let workflow_input =
                        match crate::services::product_workflows::expand_matching_snippet(
                            &voice_snippets,
                            &text,
                            &snippet_context,
                            &chrono::Local::now().format("%Y-%m-%d").to_string(),
                        ) {
                            Ok(Some(expanded)) => expanded,
                            Ok(None) => text,
                            Err(error) => {
                                warn!(task_id, error = %error, "voice_snippet_expansion_failed");
                                crate::events::emit_pill_tooltip(
                                    &app_clone,
                                    format!("Snippet not expanded: {error}"),
                                    4_000,
                                    Some(task_id),
                                );
                                raw_text.clone()
                            }
                        };
                    let postprocess = apply_post_stt_processing(
                        &workflow_input,
                        correction_memory_enabled,
                        &user_glossary,
                        &custom_dictionary,
                        task_id,
                        "recording",
                    );
                    let state = app_clone.state::<AppState>();
                    if state.is_cancellation_requested(task_id) {
                        discard_canceled_result(&state, task_id, audio_path.as_ref());
                        return;
                    }
                    let polish_result = if postprocess.text.is_empty() {
                        PolishProcessingResult::skipped(String::new(), "empty postprocess text")
                    } else {
                        let workflow_profile = app_clone
                            .try_state::<crate::services::product_workflows::WorkflowRuntime>()
                            .and_then(|runtime| runtime.profile_for_task(task_id));
                        maybe_polish_transcription_text_for_profile(
                            &ProcessingEventTarget::Recording(&app_clone),
                            &state,
                            task_id,
                            postprocess.text,
                            resolved_polish_template_id_clone.clone(),
                            workflow_profile.as_ref(),
                            vibe_context.as_ref(),
                        )
                        .await
                    };
                    let polish_time_ms = polish_result.polish_ms;
                    let workflow_profile = app_clone
                        .try_state::<crate::services::product_workflows::WorkflowRuntime>()
                        .and_then(|runtime| runtime.profile_for_task(task_id));
                    let final_text = if vibe_context.is_some()
                        || workflow_profile
                            .as_ref()
                            .is_some_and(|profile| profile.code_aware)
                    {
                        crate::services::platform_quality::format_code_aware_transcript(
                            &polish_result.text,
                            workflow_profile
                                .as_ref()
                                .and_then(|profile| profile.language.as_deref()),
                        )
                    } else {
                        polish_result.text
                    };

                    if state.is_cancellation_requested(task_id) {
                        discard_canceled_result(&state, task_id, audio_path.as_ref());
                        return;
                    }

                    let quality_event = transcription_quality_event(
                        quality_application_id.as_deref(),
                        &final_text,
                        stt_wall_ms,
                        polish_time_ms,
                        cloud_stt_enabled,
                    );
                    crate::commands::platform_quality::record_quality_event(&quality_event);

                    info!(
                        task_id,
                        text_len = final_text.len(),
                        postprocess_ms = postprocess.postprocess_ms,
                        normalization_applied = postprocess.normalization_applied,
                        corrections_applied = postprocess.corrections_applied,
                        hotwords_applied = postprocess.hotwords_applied,
                        glossary_applied = postprocess.glossary_applied,
                        polish_ms = polish_time_ms,
                        polish_wall_ms = polish_result.polish_wall_ms,
                        polish_queue_ms = polish_result.polish_queue_ms,
                        model_load_ms = polish_result.model_load_ms,
                        context_create_ms = polish_result.context_create_ms,
                        prefill_ms = polish_result.prefill_ms,
                        inference_ms = polish_result.inference_ms,
                        time_to_first_token_ms = polish_result.time_to_first_token_ms,
                        generation_ms = polish_result.generation_ms,
                        fallback_reason = polish_result.fallback_reason.unwrap_or(""),
                        audio_saved = audio_path.is_some(),
                        "transcription_final_received"
                    );

                    let delivery_plan = workflow_delivery_plan(
                        workflow_profile
                            .as_ref()
                            .map(|profile| profile.output_action),
                    );
                    if !final_text.is_empty() {
                        if let Some(runtime) = app_clone
                            .try_state::<crate::services::product_workflows::WorkflowRuntime>(
                        ) {
                            if delivery_plan == WorkflowDeliveryPlan::Insert {
                                runtime.stage_delivery(
                                    task_id,
                                    crate::services::product_workflows::DeliveryRecord {
                                        raw_text: raw_text.clone(),
                                        final_text: final_text.clone(),
                                        inserted_text: final_text.clone(),
                                        application_id: runtime
                                            .context()
                                            .and_then(|context| context.application_id),
                                        created_at_ms: chrono::Utc::now().timestamp_millis(),
                                    },
                                );
                            } else if delivery_plan == WorkflowDeliveryPlan::Preview {
                                runtime.set_preview(
                                    crate::services::product_workflows::VoiceActionPreview {
                                        kind: if workflow_profile
                                            .as_ref()
                                            .and_then(|profile| profile.translation_target.as_ref())
                                            .is_some()
                                        {
                                            crate::services::product_workflows::VoiceActionKind::Translate
                                        } else {
                                            crate::services::product_workflows::VoiceActionKind::Custom
                                        },
                                        source_text: raw_text.clone(),
                                        result_text: final_text.clone(),
                                        translation_target: workflow_profile
                                            .as_ref()
                                            .and_then(|profile| profile.translation_target.clone()),
                                        output_action:
                                            crate::services::product_workflows::OutputAction::Preview,
                                    },
                                );
                            }
                        }
                    }
                    let copy_delivery_succeeded =
                        if delivery_plan == WorkflowDeliveryPlan::Copy && !final_text.is_empty() {
                            match app_clone.clipboard().write_text(&final_text) {
                                Ok(()) => true,
                                Err(error) => {
                                    warn!(task_id, error = %error, "workflow_copy_delivery_failed");
                                    crate::events::emit_pill_tooltip(
                                        &app_clone,
                                        format!("Could not copy transcription: {error}"),
                                        4_000,
                                        Some(task_id),
                                    );
                                    false
                                }
                            }
                        } else {
                            false
                        };
                    if !final_text.is_empty()
                        && (delivery_plan == WorkflowDeliveryPlan::Preview
                            || (delivery_plan == WorkflowDeliveryPlan::Copy
                                && copy_delivery_succeeded))
                    {
                        if let Some(runtime) = app_clone
                            .try_state::<crate::services::product_workflows::WorkflowRuntime>(
                        ) {
                            runtime.record_delivery(
                                crate::services::product_workflows::DeliveryRecord {
                                    raw_text: raw_text.clone(),
                                    final_text: final_text.clone(),
                                    inserted_text: String::new(),
                                    application_id: runtime
                                        .context()
                                        .and_then(|context| context.application_id),
                                    created_at_ms: chrono::Utc::now().timestamp_millis(),
                                },
                            );
                        }
                    }
                    let action = if !final_text.is_empty() {
                        let delivery_disposition = match delivery_plan {
                            WorkflowDeliveryPlan::Insert => DeliveryDisposition::Insert,
                            WorkflowDeliveryPlan::Preview => DeliveryDisposition::Preview,
                            WorkflowDeliveryPlan::Copy if copy_delivery_succeeded => {
                                DeliveryDisposition::Copied
                            }
                            WorkflowDeliveryPlan::Copy => DeliveryDisposition::CopyFailed,
                        };
                        finalize_successful_transcription_for_output(
                            &state,
                            &raw_text,
                            &final_text,
                            polish_time_ms,
                            audio_path.clone(),
                            delivery_disposition,
                        )
                    } else {
                        finalize_empty_transcription(&state, audio_path)
                    };
                    let _ = state.finish_session(task_id);
                    apply_finalize_result(
                        &app_clone,
                        task_id,
                        action,
                        original_target.enabled,
                        original_target.mode,
                        original_target.target.as_ref(),
                    )
                    .await;
                }
                Err(e) => {
                    let state = app_clone.state::<AppState>();
                    if state.is_cancellation_requested(task_id) {
                        discard_canceled_result(&state, task_id, audio_path.as_ref());
                        return;
                    }
                    error!(task_id, error = %e, "stt_finish_failed");
                    crate::commands::platform_quality::record_quality_event(
                        &crate::services::platform_quality::QualityEvent::transcription_failure(
                            quality_application_id.as_deref(),
                            stt_wall_ms,
                            cloud_stt_enabled,
                        ),
                    );

                    let action = finalize_failed_transcription(&state, audio_path, &e);
                    let _ = state.finish_session(task_id);
                    apply_finalize_result(
                        &app_clone,
                        task_id,
                        action,
                        original_target.enabled,
                        original_target.mode,
                        original_target.target.as_ref(),
                    )
                    .await;
                }
            }
        }

        let final_state = app_clone.state::<AppState>();
        final_state.is_transcribing.store(false, Ordering::SeqCst);
        let active_task_id = final_state.task_counter.load(Ordering::SeqCst);
        let cancellation_requested = final_state.is_cancellation_requested(task_id);
        if should_unregister_cancel_hotkey_after_async_cleanup(
            active_task_id,
            task_id,
            cancellation_requested,
        ) {
            if let Some(sm) = app_clone.try_state::<crate::shortcut::ShortcutManager>() {
                let _ = sm.unregister_cancel_for_task(task_id);
            }
        }
    });

    if let Some(stt) = state.streaming_stt.lock().as_mut() {
        stt.streaming_task.lock().replace(handle);
    }

    info!(
        task_id,
        sample_rate = sr,
        channels = ch,
        cloud = cloud_stt_enabled,
        "recording_started-unified"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::transcription_quality_event;
    use crate::services::platform_quality::QualityEventKind;

    #[test]
    fn recording_pipeline_builds_content_free_local_and_cloud_quality_events() {
        let success = transcription_quality_event(
            Some("com.example.editor"),
            "private transcript content",
            120,
            30,
            false,
        );
        let failure = transcription_quality_event(Some("com.example.browser"), "", 80, 0, true);

        assert_eq!(success.kind, QualityEventKind::TranscriptionSuccess);
        assert_eq!(
            success.application_id.as_deref(),
            Some("com.example.editor")
        );
        assert_eq!(success.stt_ms, Some(120));
        assert_eq!(success.polish_ms, Some(30));
        assert_eq!(success.total_ms, Some(150));
        assert_eq!(success.is_cloud, Some(false));
        assert_eq!(failure.kind, QualityEventKind::TranscriptionFailure);
        assert_eq!(failure.is_cloud, Some(true));
        assert!(!serde_json::to_string(&success)
            .expect("quality event should serialize")
            .contains("private transcript content"));
    }
}
