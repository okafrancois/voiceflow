use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::events::EventName;
use crate::history::{HistoryFilter, RetentionPolicy};
use crate::permissions::{PermissionKind, PermissionStatus};
use crate::services::platform_quality::store::{QualityQuery, QualityStore};
use crate::services::platform_quality::{
    authorize_loopback_bridge, bridge_endpoint_path, bridge_request_name,
    build_code_aware_instruction, build_diagnostic_report, clear_active_code_context,
    format_code_aware_transcript, get_active_code_context, parse_bridge_url, preset_contract,
    set_active_code_context, BridgeEndpoint, BridgeEnvelope, BridgeRequest, BridgeResponse,
    CodeContext, DiagnosticInput, DiagnosticReport, HardwareSnapshot, LastTextVersion,
    LatencySample, MicrophoneCheck, QualityEvent, QualitySummary, SetupPreset,
};
use crate::state::app_state::AppState;
use crate::stt_engine::TranscriptionRequest;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeStatus {
    pub recording_state: String,
    pub is_recording: bool,
    pub is_transcribing: bool,
    pub last_result_available: bool,
    pub active_code_context: Option<CodeContext>,
}

#[tauri::command]
pub async fn run_setup_diagnostics(
    state: State<'_, AppState>,
    microphone_sample_ms: Option<u64>,
) -> Result<DiagnosticReport, String> {
    let sample_ms = microphone_sample_ms.unwrap_or(0).min(3_000);
    let permission = crate::permissions::check_permission(PermissionKind::Microphone);
    let microphone = if sample_ms > 0 && permission != PermissionStatus::Granted {
        MicrophoneCheck {
            ready: false,
            device_name: None,
            sample_rate_hz: None,
            channels: None,
            peak_level: None,
            error: Some(format!("Microphone permission is {}", permission.as_str())),
        }
    } else {
        let probe = tauri::async_runtime::spawn_blocking(move || {
            crate::sensors::setup_diagnostics::probe_microphone(sample_ms)
        })
        .await
        .map_err(|error| format!("Microphone diagnostic task failed: {error}"))?;
        MicrophoneCheck {
            ready: probe.ready,
            device_name: probe.device_name,
            sample_rate_hz: probe.sample_rate_hz,
            channels: probe.channels,
            peak_level: probe.peak_level,
            error: probe.error,
        }
    };
    let hardware = crate::sensors::setup_diagnostics::probe_hardware();
    let has_cloud_credentials = {
        let settings = state.settings.lock();
        settings.cloud_stt_configs.values().any(|config| {
            !config.api_key.trim().is_empty()
                && (config.provider_type != "volcengine-streaming"
                    || !config.app_id.trim().is_empty())
        })
    };
    Ok(build_diagnostic_report(DiagnosticInput {
        microphone,
        hardware: HardwareSnapshot {
            total_memory_mb: hardware.total_memory_mb,
            logical_cpu_count: hardware.logical_cpu_count,
            architecture: hardware.architecture,
        },
        has_cloud_credentials,
        latency: None,
    }))
}

#[tauri::command]
pub async fn run_setup_latency_test(
    state: State<'_, AppState>,
    media_path: String,
) -> Result<LatencySample, String> {
    let path = PathBuf::from(media_path);
    let decoded = tauri::async_runtime::spawn_blocking(move || {
        crate::services::transcription_workbench::decode_media_to_mono_16k(&path)
    })
    .await
    .map_err(|error| format!("Latency test decode task failed: {error}"))??;
    let (requested_model, language) = {
        let settings = state.settings.lock();
        (settings.model.clone(), settings.stt_engine_language.clone())
    };
    let (engine, model_name) = state
        .engine_manager
        .resolve_available_model(&requested_model, &language);
    let request = TranscriptionRequest::new(decoded.samples)
        .with_model(model_name.clone())
        .with_language(language);
    let started = Instant::now();
    let result = state.engine_manager.transcribe(engine, request).await?;
    Ok(LatencySample {
        stt_ms: result.total_ms,
        polish_ms: None,
        total_ms: elapsed_millis(started),
        model_name,
    })
}

#[tauri::command]
pub fn apply_setup_preset(
    app: AppHandle,
    state: State<'_, AppState>,
    preset: SetupPreset,
) -> Result<crate::commands::settings::AppSettings, String> {
    let contract = preset_contract(preset);
    let previous = state.settings.lock().clone();
    {
        let mut settings = state.settings.lock();
        if contract.cloud_stt_enabled {
            let active = settings.get_active_cloud_stt_config();
            if active.api_key.trim().is_empty()
                || (settings.active_cloud_stt_provider == "volcengine-streaming"
                    && active.app_id.trim().is_empty())
            {
                return Err(
                    "Maximum Accuracy requires valid credentials for the active cloud STT provider"
                        .to_string(),
                );
            }
        }
        settings.cloud_stt_enabled = contract.cloud_stt_enabled;
        settings.context_capture.application_metadata = contract.window_context_enabled;
        settings.context_capture.focused_field = contract.window_context_enabled;
        settings.context_capture.selected_text = contract.window_context_enabled;
        settings.context_capture.clipboard = contract.clipboard_context_enabled;
        settings.context_capture.ocr_fallback = contract.ocr_fallback_enabled;
        settings.window_context_enabled = contract.ocr_fallback_enabled;
        settings.correction_memory_enabled = contract.correction_memory_enabled;
        settings.text_retention = retention_from_contract(&contract.text_retention)?;
        settings.audio_retention = retention_from_contract(&contract.audio_retention)?;
    }

    if let Err(error) = crate::commands::settings::save_settings_internal(&app) {
        *state.settings.lock() = previous;
        return Err(error);
    }
    let settings = state.settings.lock().clone();
    let _ = app.emit(EventName::SETTINGS_CHANGED, settings.clone());
    let cleanup = state
        .history_store
        .lock()
        .cleanup_retention(settings.text_retention, settings.audio_retention);
    if let Err(error) = cleanup {
        tracing::warn!(error = %error, "setup_preset_retention_cleanup_failed");
    }
    Ok(settings)
}

#[tauri::command]
pub fn set_code_context(context: CodeContext) -> Result<CodeContext, String> {
    set_active_code_context(context)
}

#[tauri::command]
pub fn get_code_context() -> Result<Option<CodeContext>, String> {
    get_active_code_context()
}

#[tauri::command]
pub fn clear_code_context() -> Result<(), String> {
    clear_active_code_context()
}

#[tauri::command]
pub fn format_code_transcript(text: String, language: Option<String>) -> String {
    format_code_aware_transcript(&text, language.as_deref())
}

#[tauri::command]
pub fn get_quality_summary(query: QualityQuery) -> Result<QualitySummary, String> {
    QualityStore::new()?.summary(&query)
}

#[tauri::command]
pub fn get_quality_events(query: QualityQuery) -> Result<Vec<QualityEvent>, String> {
    QualityStore::new()?.query(&query)
}

#[tauri::command]
pub fn clear_quality_metrics() -> Result<usize, String> {
    QualityStore::new()?.clear()
}

#[tauri::command]
pub fn export_quality_metrics(
    path: String,
    query: QualityQuery,
    overwrite: bool,
) -> Result<String, String> {
    QualityStore::new()?
        .export_to_file(Path::new(&path), &query, overwrite)
        .map(|path| path.display().to_string())
}

pub fn record_quality_event(event: &QualityEvent) {
    if let Err(error) = QualityStore::new().and_then(|store| store.record(event)) {
        tracing::warn!(error = %error, kind = ?event.kind, "quality_event_record_failed");
    }
}

#[tauri::command]
pub async fn execute_bridge_url(app: AppHandle, url: String) -> Result<BridgeResponse, String> {
    let request = parse_bridge_url(&url)?;
    Ok(execute_bridge_request(app, request).await)
}

pub fn dispatch_bridge_url(app: AppHandle, url: &str) -> Result<(), String> {
    let request = parse_bridge_url(url)?;
    tauri::async_runtime::spawn(async move {
        let response = execute_bridge_request(app, request).await;
        if !response.ok {
            tracing::warn!(
                command = %response.command,
                error = response.error.as_deref().unwrap_or("unknown"),
                "deep_link_command_failed"
            );
        }
    });
    Ok(())
}

pub async fn execute_bridge_request(app: AppHandle, request: BridgeRequest) -> BridgeResponse {
    let command = bridge_request_name(&request);
    let result = execute_bridge_request_inner(&app, &request).await;
    match result {
        Ok(data) => {
            tracing::info!(command, "developer_bridge_command_complete");
            BridgeResponse::success(&request, data)
        }
        Err(error) => {
            tracing::warn!(command, error = %error, "developer_bridge_command_failed");
            BridgeResponse::failure(&request, error)
        }
    }
}

async fn execute_bridge_request_inner(
    app: &AppHandle,
    request: &BridgeRequest,
) -> Result<Option<serde_json::Value>, String> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| "AppState not available".to_string())?;
    match request {
        BridgeRequest::Start { profile_id } => {
            let profile = {
                let settings = state.settings.lock();
                profile_id
                    .as_deref()
                    .map(|id| {
                        settings
                            .workflow_profiles
                            .iter()
                            .find(|profile| profile.id == id)
                            .map(|profile| profile.shortcut_profile())
                            .ok_or_else(|| format!("Unknown workflow profile: {id}"))
                    })
                    .transpose()?
            };
            crate::commands::audio::start_recording_sync(app, profile.as_ref())?;
            Ok(None)
        }
        BridgeRequest::Stop => {
            crate::commands::audio::stop_recording_sync(app.clone())?;
            Ok(None)
        }
        BridgeRequest::Cancel => {
            crate::commands::audio::cancel_recording_sync(app.clone())?;
            Ok(None)
        }
        BridgeRequest::Status => {
            let last_result_available = !state
                .history_store
                .lock()
                .get_history(&latest_history_filter())?
                .is_empty();
            let status = BridgeStatus {
                recording_state: state.get_current_state().as_str().to_string(),
                is_recording: state.is_recording.load(Ordering::SeqCst),
                is_transcribing: state.is_transcribing.load(Ordering::SeqCst),
                last_result_available,
                active_code_context: get_active_code_context()?,
            };
            Ok(Some(
                serde_json::to_value(status).map_err(|error| error.to_string())?,
            ))
        }
        BridgeRequest::TranscribeFile { path, profile_id } => {
            let result = transcribe_bridge_file(&state, path, profile_id.as_deref()).await?;
            Ok(Some(serde_json::json!({ "text": result })))
        }
        BridgeRequest::Insert { text } => {
            let injection_started = Instant::now();
            if let Err(error) = crate::commands::text::do_insert_text(text) {
                record_quality_event(&QualityEvent::injection_failure(
                    workflow_application_id(app).as_deref(),
                    elapsed_millis(injection_started),
                ));
                return Err(error);
            }
            Ok(None)
        }
        BridgeRequest::CopyLast { version } => {
            let text = latest_text(&state, *version)?;
            app.clipboard()
                .write_text(&text)
                .map_err(|error| error.to_string())?;
            Ok(Some(
                serde_json::json!({ "copied": true, "characters": text.chars().count() }),
            ))
        }
        BridgeRequest::ReinsertLast { version } => {
            let text = latest_text(&state, *version)?;
            let injection_started = Instant::now();
            if let Err(error) = crate::commands::text::do_insert_text(&text) {
                record_quality_event(&QualityEvent::injection_failure(
                    workflow_application_id(app).as_deref(),
                    elapsed_millis(injection_started),
                ));
                return Err(error);
            }
            Ok(Some(
                serde_json::json!({ "characters": text.chars().count() }),
            ))
        }
        BridgeRequest::Submit => {
            crate::text_injector::quick_controls::send_enter()?;
            Ok(None)
        }
        BridgeRequest::SetCodeContext { context } => {
            let context = set_active_code_context(context.clone())?;
            Ok(Some(
                serde_json::to_value(context).map_err(|error| error.to_string())?,
            ))
        }
        BridgeRequest::ClearCodeContext => {
            clear_active_code_context()?;
            Ok(None)
        }
        BridgeRequest::FormatCode { text, language } => Ok(Some(serde_json::json!({
            "text": format_code_aware_transcript(text, language.as_deref())
        }))),
    }
}

async fn transcribe_bridge_file(
    state: &AppState,
    path: &str,
    profile_id: Option<&str>,
) -> Result<String, String> {
    let path = PathBuf::from(path);
    let decoded = tauri::async_runtime::spawn_blocking(move || {
        crate::services::transcription_workbench::decode_media_to_mono_16k(&path)
    })
    .await
    .map_err(|error| format!("Media decode task failed: {error}"))??;
    let (requested_model, language, code_aware) = {
        let settings = state.settings.lock();
        let profile = profile_id
            .map(|id| {
                settings
                    .workflow_profiles
                    .iter()
                    .find(|profile| profile.id == id)
                    .ok_or_else(|| format!("Unknown workflow profile: {id}"))
            })
            .transpose()?;
        (
            settings.model.clone(),
            profile
                .and_then(|profile| profile.language.clone())
                .unwrap_or_else(|| settings.stt_engine_language.clone()),
            profile.is_some_and(|profile| profile.code_aware),
        )
    };
    let (engine, model_name) = state
        .engine_manager
        .resolve_available_model(&requested_model, &language);
    let mut request = TranscriptionRequest::new(decoded.samples)
        .with_model(model_name)
        .with_language(language.clone());
    if code_aware {
        let context = get_active_code_context()?.unwrap_or_default();
        request = request.with_prompt(build_code_aware_instruction(&context));
    }
    let result = state.engine_manager.transcribe(engine, request).await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            record_quality_event(&QualityEvent::transcription_failure(None, 0, false));
            return Err(error);
        }
    };
    let final_text = if code_aware {
        format_code_aware_transcript(&result.text, Some(&language))
    } else {
        result.text.clone()
    };
    crate::history::save_to_history_with_delivery_status(
        state,
        crate::history::HistorySaveRequest {
            raw_text: &result.text,
            final_text: &final_text,
            stt_duration_ms: Some(i64::try_from(result.total_ms).unwrap_or(i64::MAX)),
            polish_duration_ms: None,
            polish_applied: false,
            audio_path: None,
            delivery_status: "not_delivered",
        },
    );
    record_quality_event(&QualityEvent::success_with_source(
        None,
        result.total_ms,
        0,
        result.total_ms,
        false,
    ));
    Ok(final_text)
}

fn latest_text(state: &AppState, version: LastTextVersion) -> Result<String, String> {
    let entry = state
        .history_store
        .lock()
        .get_history(&latest_history_filter())?
        .into_iter()
        .next()
        .ok_or_else(|| "No previous transcription is available".to_string())?;
    Ok(match version {
        LastTextVersion::Raw => entry.raw_text,
        LastTextVersion::Final => entry.final_text,
    })
}

fn latest_history_filter() -> HistoryFilter {
    HistoryFilter {
        search: None,
        engine: None,
        status: Some("success".to_string()),
        date_from: None,
        date_to: None,
        limit: Some(1),
        offset: Some(0),
    }
}

fn workflow_application_id(app: &AppHandle) -> Option<String> {
    app.try_state::<crate::services::product_workflows::WorkflowRuntime>()
        .and_then(|runtime| runtime.context())
        .and_then(|context| context.application_id)
}

pub fn start_developer_bridge(app: AppHandle) -> Result<BridgeEndpoint, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("Failed to bind developer bridge: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("Failed to read developer bridge address: {error}"))?;
    let endpoint = BridgeEndpoint {
        protocol_version: 1,
        address: address.to_string(),
        token: uuid::Uuid::new_v4().simple().to_string(),
        process_id: std::process::id(),
    };
    write_bridge_endpoint(&endpoint)?;
    let server_endpoint = endpoint.clone();
    std::thread::Builder::new()
        .name("voice-flow-developer-bridge".to_string())
        .spawn(move || {
            for connection in listener.incoming() {
                match connection {
                    Ok(stream) => handle_bridge_connection(&app, &server_endpoint, stream),
                    Err(error) => {
                        tracing::warn!(error = %error, "developer_bridge_accept_failed")
                    }
                }
            }
        })
        .map_err(|error| format!("Failed to start developer bridge thread: {error}"))?;
    Ok(endpoint)
}

fn handle_bridge_connection(app: &AppHandle, endpoint: &BridgeEndpoint, mut stream: TcpStream) {
    let response = (|| -> Result<BridgeResponse, String> {
        let peer = stream
            .peer_addr()
            .map_err(|error| format!("Failed to read developer bridge peer: {error}"))?;
        let mut line = String::new();
        BufReader::new(
            stream
                .try_clone()
                .map_err(|error| format!("Failed to read developer bridge request: {error}"))?,
        )
        .take(256 * 1_024)
        .read_line(&mut line)
        .map_err(|error| format!("Failed to read developer bridge request: {error}"))?;
        let envelope = serde_json::from_str::<BridgeEnvelope>(line.trim())
            .map_err(|error| format!("Invalid developer bridge request: {error}"))?;
        authorize_loopback_bridge(&peer.ip().to_string(), &envelope.token, &endpoint.token)?;
        Ok(tauri::async_runtime::block_on(execute_bridge_request(
            app.clone(),
            envelope.request,
        )))
    })();
    let response = response.unwrap_or_else(|error| BridgeResponse {
        ok: false,
        command: "invalid".to_string(),
        data: None,
        error: Some(error),
    });
    if serde_json::to_writer(&mut stream, &response).is_ok() {
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
    }
}

fn write_bridge_endpoint(endpoint: &BridgeEndpoint) -> Result<(), String> {
    let path = bridge_endpoint_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create developer bridge directory: {error}"))?;
    }
    let bytes = serde_json::to_vec_pretty(endpoint)
        .map_err(|error| format!("Failed to encode developer bridge endpoint: {error}"))?;
    std::fs::write(&path, bytes)
        .map_err(|error| format!("Failed to write developer bridge endpoint: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Failed to secure developer bridge endpoint: {error}"))?;
    }
    Ok(())
}

fn retention_from_contract(value: &str) -> Result<RetentionPolicy, String> {
    match value {
        "never" => Ok(RetentionPolicy::Never),
        "days7" => Ok(RetentionPolicy::Days7),
        "days30" => Ok(RetentionPolicy::Days30),
        "days90" => Ok(RetentionPolicy::Days90),
        "forever" => Ok(RetentionPolicy::Forever),
        _ => Err(format!("Unknown retention policy: {value}")),
    }
}

fn elapsed_millis(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
