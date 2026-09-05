use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{info, warn};

use crate::commands::window::position_pill_window;
use crate::events::EventName;
use crate::history::RetentionPolicy;
use crate::services::product_workflows::{
    apply_profile_registration_transaction, default_workflow_profiles, migrate_legacy_profiles,
    ApplicationRule, ContextCaptureSettings, VoiceSnippet, WorkflowProfile,
    WorkflowProfileRegistrar,
};
use crate::shortcut::ShortcutProfilesMap;
use crate::state::app_state::AppState;
use crate::utils::AppPaths;

struct ShortcutManagerRegistrar<'a>(&'a crate::shortcut::ShortcutManager);

impl WorkflowProfileRegistrar for ShortcutManagerRegistrar<'_> {
    fn register(
        &mut self,
        id: &str,
        profile: &crate::shortcut::ShortcutProfile,
    ) -> Result<(), String> {
        self.0.register_profile(id, profile)
    }

    fn unregister(&mut self, id: &str) -> Result<(), String> {
        self.0.unregister_profile(id)
    }
}

/// Cloud provider configuration for polish
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CloudProviderConfig {
    pub enabled: bool,
    pub provider_type: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub enable_thinking: bool,
}

/// Cloud provider configuration for STT
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CloudSttConfig {
    pub enabled: bool,
    pub provider_type: String,
    pub api_key: String,
    pub app_id: String,
    pub base_url: String,
    pub model: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudConnectionCheckResult {
    pub ok: bool,
    pub kind: String,
    pub message: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct LocalPolishRuntimeSettings {
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub server_command: String,
    pub server_args_json: String,
    pub ready_timeout_secs: u64,
}

/// User-defined custom polish template
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomPolishTemplate {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
}

// Legacy config structs for migration from old format
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct LegacyCloudProviderConfig {
    pub enabled: bool,
    pub provider_type: String,
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub enable_thinking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct LegacyCloudSttConfig {
    pub enabled: bool,
    pub provider_type: String,
    pub api_key: String,
    pub app_id: String,
    pub base_url: String,
    pub model: String,
    pub language: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OriginalTargetMode {
    #[default]
    Foreground,
    Background,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// Canonical unlimited recording profiles.
    pub workflow_profiles: Vec<WorkflowProfile>,
    /// Ordered application matchers selecting a workflow profile.
    pub application_rules: Vec<ApplicationRule>,
    /// Deterministic spoken snippet expansions.
    pub voice_snippets: Vec<VoiceSnippet>,
    /// Consent switches for structured and sensitive context sources.
    pub context_capture: ContextCaptureSettings,
    pub recording_mode: String,
    pub model: String,
    pub stt_engine: String,
    pub pill_position: String,
    pub pill_indicator_mode: String,
    pub auto_start: bool,
    pub gpu_acceleration: bool,
    pub language: String,
    pub stt_engine_language: String,
    pub beep_on_record: bool,
    pub audio_device: String,
    pub polish_system_prompt: String,
    pub polish_model: String,
    pub theme_mode: String,
    pub stt_engine_initial_prompt: String,
    pub model_resident: bool,
    pub idle_unload_minutes: u32,
    pub denoise_mode: String,
    pub stt_engine_work_domain: String,
    pub stt_engine_work_domain_prompt: String,
    pub stt_engine_work_subdomain: String,
    pub stt_engine_user_glossary: String,
    pub custom_dictionary: String,
    pub analytics_opt_in: bool,
    /// Enable authenticated local automation and URL commands.
    pub developer_bridge_enabled: bool,
    /// Apply fresh, editor-scoped code context to ordinary dictation.
    pub vibe_coding_enabled: bool,
    /// How long transcription text and history metadata remain on this device.
    pub text_retention: RetentionPolicy,
    /// How long captured WAV files remain on this device.
    pub audio_retention: RetentionPolicy,
    pub cloud_stt_enabled: bool,
    pub active_cloud_stt_provider: String,
    pub cloud_stt_configs: HashMap<String, CloudSttConfig>,
    pub cloud_polish_enabled: bool,
    pub active_cloud_polish_provider: String,
    pub cloud_polish_configs: HashMap<String, CloudProviderConfig>,
    pub local_polish_runtime: LocalPolishRuntimeSettings,
    /// Deliver completed recordings to the editable target captured at start.
    pub original_target_enabled: bool,
    /// Prefer foreground compatibility or best-effort background Accessibility delivery.
    pub original_target_mode: OriginalTargetMode,
    pub vad_enabled: bool,
    #[serde(default = "default_stay_in_tray")]
    pub stay_in_tray: bool,
    pub polish_custom_templates: Vec<CustomPolishTemplate>,
    /// Enable window context capture via screenshot + OCR at recording start.
    /// When enabled, the focused window content is injected into polish prompts.
    pub window_context_enabled: bool,
    /// Pill window size level: 1-5, default 2.
    /// Controls the visual scale of the pill indicator via CSS font-size scaling.
    #[serde(default = "default_pill_size")]
    pub pill_size: u8,
    /// Pill window background color as a #RRGGBB hex value.
    #[serde(default = "default_pill_background_color")]
    pub pill_background_color: String,
    /// Pill window background opacity from 0.2 to 1.0.
    #[serde(default = "default_pill_background_opacity")]
    pub pill_background_opacity: f32,
    /// Learn bounded wrong -> corrected mappings from post-delivery edits.
    #[serde(default = "default_correction_memory_enabled")]
    pub correction_memory_enabled: bool,
}

fn default_pill_size() -> u8 {
    2
}

fn default_pill_background_color() -> String {
    "#1d1d1d".to_string()
}

fn default_pill_background_opacity() -> f32 {
    1.0
}

fn default_correction_memory_enabled() -> bool {
    true
}

fn default_stay_in_tray() -> bool {
    true
}

impl Default for LocalPolishRuntimeSettings {
    fn default() -> Self {
        Self {
            provider_type: "llama-server".to_string(),
            base_url: "http://127.0.0.1:8000/v1".to_string(),
            api_key: String::new(),
            server_command: String::new(),
            server_args_json: String::new(),
            ready_timeout_secs: 20,
        }
    }
}

const CLOUD_CONFIG_CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const LOCAL_POLISH_RUNTIME_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
struct CloudConfigValidationError {
    kind: &'static str,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalPolishRuntimeSettingAction {
    None,
    StopManagedRuntime,
}

impl CloudConnectionCheckResult {
    fn success(message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            ok: true,
            kind: "ok".to_string(),
            message: message.into(),
            duration_ms,
        }
    }

    fn failure(kind: impl Into<String>, message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            ok: false,
            kind: kind.into(),
            message: message.into(),
            duration_ms,
        }
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn validate_cloud_url(url: &str) -> bool {
    url.trim().is_empty() || reqwest::Url::parse(url.trim()).is_ok()
}

fn validate_required_url(url: &str) -> bool {
    !url.trim().is_empty() && reqwest::Url::parse(url.trim()).is_ok()
}

fn stt_config_field_value<'a>(config: &'a CloudSttConfig, key: &str) -> Option<&'a str> {
    match key {
        "api_key" => Some(&config.api_key),
        "app_id" => Some(&config.app_id),
        "base_url" => Some(&config.base_url),
        "model" => Some(&config.model),
        "language" => Some(&config.language),
        _ => None,
    }
}

fn polish_config_field_value<'a>(config: &'a CloudProviderConfig, key: &str) -> Option<&'a str> {
    match key {
        "api_key" => Some(&config.api_key),
        "base_url" => Some(&config.base_url),
        "model" => Some(&config.model),
        _ => None,
    }
}

fn validate_cloud_stt_config_for_check(
    config: &CloudSttConfig,
) -> Result<(), CloudConfigValidationError> {
    let Some(schema) = crate::provider_schema::STT_SCHEMAS
        .iter()
        .find(|schema| schema.id == config.provider_type)
    else {
        return Err(CloudConfigValidationError {
            kind: "unsupported_provider",
            message: format!("Unsupported cloud STT provider: {}", config.provider_type),
        });
    };

    for field in schema.fields {
        if field.required {
            let value = stt_config_field_value(config, field.key).unwrap_or_default();
            if value.trim().is_empty() {
                return Err(CloudConfigValidationError {
                    kind: "missing_required",
                    message: format!("Missing required field: {}", field.name),
                });
            }
        }
    }

    if !validate_cloud_url(&config.base_url) {
        return Err(CloudConfigValidationError {
            kind: "invalid_url",
            message: "Invalid Base URL format".to_string(),
        });
    }

    Ok(())
}

fn validate_cloud_polish_config_for_check(
    config: &CloudProviderConfig,
) -> Result<(), CloudConfigValidationError> {
    let Some(schema) = crate::provider_schema::POLISH_SCHEMAS
        .iter()
        .find(|schema| schema.id == config.provider_type)
    else {
        return Err(CloudConfigValidationError {
            kind: "unsupported_provider",
            message: format!(
                "Unsupported cloud polish provider: {}",
                config.provider_type
            ),
        });
    };

    for field in schema.fields {
        if field.required {
            let value = polish_config_field_value(config, field.key).unwrap_or_default();
            if value.trim().is_empty() {
                return Err(CloudConfigValidationError {
                    kind: "missing_required",
                    message: format!("Missing required field: {}", field.name),
                });
            }
        }
    }

    if !validate_cloud_url(&config.base_url) {
        return Err(CloudConfigValidationError {
            kind: "invalid_url",
            message: "Invalid Base URL format".to_string(),
        });
    }

    Ok(())
}

fn classify_cloud_check_error(error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();

    if lower.contains("timed out") || lower.contains("timeout") {
        return "timeout";
    }

    if lower.contains("401")
        || lower.contains("403")
        || lower.contains("unauthorized")
        || lower.contains("forbidden")
        || lower.contains("invalid_api_key")
        || lower.contains("authentication")
        || lower.contains("api key")
        || lower.contains("access token")
    {
        return "auth_failed";
    }

    if lower.contains("model") {
        return "model_failed";
    }

    if lower.contains("invalid url") {
        return "invalid_url";
    }

    if lower.contains("unsupported") {
        return "unsupported_provider";
    }

    if lower.contains("dns")
        || lower.contains("network")
        || lower.contains("connection")
        || lower.contains("connect")
        || lower.contains("lookup")
    {
        return "network_failed";
    }

    "provider_error"
}

fn cloud_check_user_message(kind: &str) -> &'static str {
    match kind {
        "auth_failed" => "Authentication failed. Check credentials and provider permissions.",
        "invalid_url" => "The endpoint URL is invalid.",
        "model_failed" => "The provider rejected the configured model.",
        "network_failed" => "Could not reach the provider endpoint.",
        "timeout" => "Connection check timed out.",
        "unsupported_provider" => "This provider is not supported by the checker.",
        _ => "The provider rejected the connection check.",
    }
}

fn normalize_pill_background_color(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() != 7 || bytes.first() != Some(&b'#') {
        return None;
    }

    if bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        Some(trimmed.to_ascii_lowercase())
    } else {
        None
    }
}

fn normalize_pill_background_opacity(value: f64) -> Option<f32> {
    if !value.is_finite() {
        return None;
    }

    Some(value.clamp(0.2, 1.0) as f32)
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            workflow_profiles: default_workflow_profiles(),
            application_rules: Vec::new(),
            voice_snippets: Vec::new(),
            context_capture: ContextCaptureSettings::default(),
            recording_mode: "hold".to_string(),
            model: "whisper-base".to_string(),
            stt_engine: "whisper".to_string(),
            pill_position: "bottom-center".to_string(),
            pill_indicator_mode: "when_recording".to_string(),
            auto_start: false,
            gpu_acceleration: true,
            language: "auto".to_string(),
            stt_engine_language: "auto".to_string(),
            beep_on_record: true,
            audio_device: "default".to_string(),
            polish_system_prompt: crate::polish_engine::DEFAULT_POLISH_PROMPT.to_string(),
            polish_model: String::new(),
            theme_mode: "light".to_string(),
            stt_engine_initial_prompt: String::new(),
            model_resident: true,
            idle_unload_minutes: 5,
            denoise_mode: "off".to_string(),
            stt_engine_work_domain: "general".to_string(),
            stt_engine_work_domain_prompt: String::new(),
            stt_engine_work_subdomain: String::new(),
            stt_engine_user_glossary: String::new(),
            custom_dictionary: String::new(),
            analytics_opt_in: false,
            developer_bridge_enabled: false,
            vibe_coding_enabled: false,
            text_retention: RetentionPolicy::Days90,
            audio_retention: RetentionPolicy::Never,
            cloud_stt_enabled: false,
            active_cloud_stt_provider: "volcengine-streaming".to_string(),
            cloud_stt_configs: HashMap::new(),
            cloud_polish_enabled: false,
            active_cloud_polish_provider: "anthropic".to_string(),
            cloud_polish_configs: HashMap::new(),
            local_polish_runtime: LocalPolishRuntimeSettings::default(),
            original_target_enabled: false,
            original_target_mode: OriginalTargetMode::Foreground,
            vad_enabled: false,
            stay_in_tray: default_stay_in_tray(),
            polish_custom_templates: Vec::new(),
            window_context_enabled: false,
            pill_size: 2,
            pill_background_color: default_pill_background_color(),
            pill_background_opacity: default_pill_background_opacity(),
            correction_memory_enabled: default_correction_memory_enabled(),
        }
    }
}

impl AppSettings {
    pub fn get_active_cloud_stt_config(&self) -> CloudSttConfig {
        let mut config = self
            .cloud_stt_configs
            .get(&self.active_cloud_stt_provider)
            .cloned()
            .unwrap_or_default();
        config.enabled = self.cloud_stt_enabled;
        config.provider_type = self.active_cloud_stt_provider.clone();
        config
    }

    pub fn get_active_cloud_polish_config(&self) -> CloudProviderConfig {
        self.cloud_polish_configs
            .get(&self.active_cloud_polish_provider)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if any streaming cloud STT provider is active
    pub fn is_streaming_stt_active(&self) -> bool {
        self.cloud_stt_enabled
            && matches!(
                self.active_cloud_stt_provider.as_str(),
                "volcengine-streaming" | "aliyun-stream" | "elevenlabs"
            )
    }

    #[deprecated(note = "Use is_streaming_stt_active instead")]
    pub fn is_volcengine_streaming_active(&self) -> bool {
        self.is_streaming_stt_active()
    }

    pub fn get_dictate_hotkey(&self) -> String {
        self.workflow_profiles
            .iter()
            .find(|profile| profile.id == "dictate")
            .map(|profile| profile.hotkey.clone())
            .unwrap_or_else(|| crate::shortcut::default_dictate_hotkey().to_string())
    }

    /// Resolve polish provider config.
    ///
    /// Provider resolution order:
    /// 1. Check active_cloud_polish_provider in cloud_polish_configs
    /// 2. If valid (api_key + model non-empty) → use cloud
    /// 3. Otherwise → local fallback
    pub fn resolve_polish_config(
        &self,
        provider_override: Option<&str>,
        model_override: Option<&str>,
    ) -> (Option<String>, CloudProviderConfig) {
        match provider_override {
            Some(provider_key) => match self.cloud_polish_configs.get(provider_key) {
                Some(cfg) if !cfg.api_key.is_empty() && !cfg.model.is_empty() => {
                    let mut resolved = cfg.clone();
                    resolved.enabled = true;
                    resolved.provider_type = provider_key.to_string();
                    if let Some(m) = model_override.filter(|m| !m.is_empty()) {
                        resolved.model = m.to_string();
                    }
                    (Some(provider_key.to_string()), resolved)
                }
                _ => {
                    tracing::warn!(
                        provider = %provider_key,
                        "polish_override_provider_invalid_fallback_to_global"
                    );
                    self.resolve_global_polish_config()
                }
            },
            None => self.resolve_global_polish_config(),
        }
    }

    fn resolve_global_polish_config(&self) -> (Option<String>, CloudProviderConfig) {
        let provider_type = &self.active_cloud_polish_provider;

        if let Some(cfg) = self.cloud_polish_configs.get(provider_type) {
            if !cfg.api_key.is_empty() && !cfg.model.is_empty() {
                return (Some(provider_type.clone()), cfg.clone());
            }
        }

        (None, CloudProviderConfig::default())
    }
}

fn get_settings_path() -> PathBuf {
    AppPaths::data_dir().join("settings.json")
}

/// Save settings to disk without requiring a specific key update.
/// Used by hotkey recording to persist the new hotkey.
pub fn save_settings_internal(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock();

    let path = get_settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&*settings).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;

    info!("settings_saved_to_disk");
    Ok(())
}

/// Migrate old cloud settings format to new per-provider format.
/// Old: single cloud_stt/cloud_polish objects with enabled/provider_type inside.
/// New: cloud_stt_enabled, active_cloud_stt_provider, cloud_stt_configs HashMap.
fn migrate_cloud_settings(json: &mut serde_json::Value) -> bool {
    let mut migrated = false;

    if let Some(old_stt) = json.get("cloud_stt").cloned() {
        if let Ok(legacy_config) = serde_json::from_value::<LegacyCloudSttConfig>(old_stt.clone()) {
            let provider_type = resolve_stt_provider_type(&legacy_config, &old_stt);

            let new_config = CloudSttConfig {
                enabled: legacy_config.enabled,
                provider_type: provider_type.clone(),
                api_key: legacy_config.api_key,
                app_id: legacy_config.app_id,
                base_url: legacy_config.base_url,
                model: legacy_config.model,
                language: legacy_config.language,
            };

            let mut configs = HashMap::new();
            configs.insert(provider_type.clone(), new_config);

            json["cloud_stt_enabled"] = serde_json::json!(legacy_config.enabled);
            json["active_cloud_stt_provider"] = serde_json::json!(provider_type);
            json["cloud_stt_configs"] =
                serde_json::to_value(&configs).unwrap_or(serde_json::json!({}));
            json.as_object_mut().map(|obj| obj.remove("cloud_stt"));

            tracing::info!(
                enabled = legacy_config.enabled,
                provider = %provider_type,
                "cloud_stt_migrated-per_provider_format"
            );
            migrated = true;
        }
    }

    if let Some(old_polish) = json.get("cloud_polish").cloned() {
        if let Ok(legacy_config) = serde_json::from_value::<LegacyCloudProviderConfig>(old_polish) {
            let provider_type = if legacy_config.provider_type.is_empty() {
                "anthropic".to_string()
            } else {
                legacy_config.provider_type.clone()
            };

            let new_config = CloudProviderConfig {
                enabled: legacy_config.enabled,
                provider_type: provider_type.clone(),
                api_key: legacy_config.api_key,
                base_url: legacy_config.base_url,
                model: legacy_config.model,
                enable_thinking: legacy_config.enable_thinking,
            };

            let mut configs = HashMap::new();
            configs.insert(provider_type.clone(), new_config);

            json["cloud_polish_enabled"] = serde_json::json!(legacy_config.enabled);
            json["active_cloud_polish_provider"] = serde_json::json!(provider_type);
            json["cloud_polish_configs"] =
                serde_json::to_value(&configs).unwrap_or(serde_json::json!({}));
            json.as_object_mut().map(|obj| obj.remove("cloud_polish"));

            tracing::info!(
                enabled = legacy_config.enabled,
                provider = %provider_type,
                "cloud_polish_migrated-per_provider_format"
            );
            migrated = true;
        }
    }

    migrated
}

fn resolve_stt_provider_type(
    legacy_config: &LegacyCloudSttConfig,
    _old_stt: &serde_json::Value,
) -> String {
    if legacy_config.provider_type == "volcengine" || legacy_config.provider_type.is_empty() {
        "volcengine-streaming".to_string()
    } else {
        legacy_config.provider_type.clone()
    }
}

fn validate_model_name(json: &mut serde_json::Value) -> bool {
    let model_value = match json.get("model").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => return false,
    };

    if crate::stt_engine::models::find_by_name(model_value).is_some() {
        return false;
    }

    tracing::info!(old = %model_value, new = "whisper-base", "model_name_reset_to_default");
    json["model"] = serde_json::Value::String("whisper-base".to_string());
    json["stt_engine"] = serde_json::Value::String("whisper".to_string());

    true
}

pub fn migrate_to_profiles_map_for_test(json: &mut serde_json::Value) {
    migrate_to_profiles_map(json);
}

fn migrate_to_profiles_map(json: &mut serde_json::Value) -> bool {
    if json.get("workflow_profiles").is_some() {
        return false;
    }
    let old_hotkey = json
        .get("hotkey")
        .and_then(|value| value.as_str())
        .unwrap_or("Shift+Space")
        .to_string();
    let legacy_recording_mode = json
        .get("recording_mode")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    if let Some(obj) = json.as_object_mut() {
        obj.remove("hotkey");
        obj.remove("polish_enabled");
        obj.remove("polish_selected_template");
    }

    // Check if shortcut_profiles is already a map
    if let Some(profiles) = json.get_mut("shortcut_profiles") {
        if profiles.is_object() {
            return ensure_profile_trigger_modes(profiles, legacy_recording_mode.as_deref());
        }

        // If array, convert to map
        if let Some(arr) = profiles.as_array() {
            let mut map = serde_json::Map::new();

            // First element → dictate
            if let Some(first) = arr.first() {
                let dictate = convert_array_item_to_profile(
                    first,
                    None,
                    profile_trigger_mode("dictate", legacy_recording_mode.as_deref()),
                );
                map.insert("dictate".to_string(), dictate);
            } else {
                map.insert(
                    "dictate".to_string(),
                    serde_json::json!({
                        "hotkey": "Shift+Space",
                        "trigger_mode": "hold",
                        "action": { "Record": { "polish_template_id": null } }
                    }),
                );
            }

            // Second element → riff
            if let Some(second) = arr.get(1) {
                let riff = convert_array_item_to_profile(
                    second,
                    Some("filler"),
                    profile_trigger_mode("riff", legacy_recording_mode.as_deref()),
                );
                map.insert("riff".to_string(), riff);
            } else {
                map.insert(
                    "riff".to_string(),
                    serde_json::json!({
                        "hotkey": "",
                        "trigger_mode": "toggle",
                        "action": { "Record": { "polish_template_id": "filler" } }
                    }),
                );
            }

            // Third element → custom (if exists)
            if let Some(third) = arr.get(2) {
                let custom = convert_array_item_to_profile(
                    third,
                    None,
                    profile_trigger_mode("custom", legacy_recording_mode.as_deref()),
                );
                map.insert("custom".to_string(), custom);
            }

            json["shortcut_profiles"] = serde_json::Value::Object(map);
            tracing::info!("shortcut_profiles_migrated-array_to_map");
            return true;
        }
    }

    json["shortcut_profiles"] = serde_json::json!({
        "dictate": {
            "hotkey": old_hotkey,
            "trigger_mode": profile_trigger_mode("dictate", legacy_recording_mode.as_deref()),
            "action": { "Record": { "polish_template_id": null } }
        },
        "riff": {
            "hotkey": "",
            "trigger_mode": profile_trigger_mode("riff", legacy_recording_mode.as_deref()),
            "action": { "Record": { "polish_template_id": "filler" } }
        }
    });

    tracing::info!(hotkey = %old_hotkey, "shortcut_profiles_migrated-from_hotkey");
    true
}

fn convert_array_item_to_profile(
    item: &serde_json::Value,
    default_template: Option<&str>,
    trigger_mode: &str,
) -> serde_json::Value {
    let hotkey = item
        .get("hotkey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let template_id = item
        .get("action")
        .and_then(|a| a.get("Record"))
        .and_then(|r| r.get("polish_template_id"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .or_else(|| default_template.map(|s| s.to_string()));

    serde_json::json!({
        "hotkey": hotkey,
        "trigger_mode": trigger_mode,
        "action": { "Record": { "polish_template_id": template_id } }
    })
}

fn ensure_profile_trigger_modes(
    profiles: &mut serde_json::Value,
    legacy_recording_mode: Option<&str>,
) -> bool {
    let Some(map) = profiles.as_object_mut() else {
        return false;
    };

    let mut migrated = false;
    for key in ["dictate", "riff", "custom"] {
        let Some(profile) = map.get_mut(key) else {
            continue;
        };
        let Some(profile_object) = profile.as_object_mut() else {
            continue;
        };
        if profile_object.contains_key("trigger_mode") {
            continue;
        }

        profile_object.insert(
            "trigger_mode".to_string(),
            serde_json::Value::String(profile_trigger_mode(key, legacy_recording_mode).to_string()),
        );
        migrated = true;
    }

    migrated
}

#[cfg(test)]
pub fn migrate_platform_shortcut_defaults_for_test(
    json: &mut serde_json::Value,
    is_macos: bool,
) -> bool {
    migrate_platform_shortcut_defaults_for_platform(json, is_macos)
}

fn migrate_platform_shortcut_defaults(json: &mut serde_json::Value) -> bool {
    migrate_platform_shortcut_defaults_for_platform(json, cfg!(target_os = "macos"))
}

fn migrate_platform_shortcut_defaults_for_platform(
    json: &mut serde_json::Value,
    is_macos: bool,
) -> bool {
    if is_macos {
        return false;
    }

    let Some(profiles) = json
        .get_mut("shortcut_profiles")
        .and_then(serde_json::Value::as_object_mut)
    else {
        return false;
    };

    let mut migrated = false;
    migrated |= migrate_profile_hotkey(profiles, "dictate", "Cmd+Slash", "Ctrl+Slash");
    migrated |= migrate_profile_hotkey(profiles, "riff", "Opt+Slash", "Alt+Slash");

    if migrated {
        tracing::info!("shortcut_profiles_migrated-platform_defaults");
    }

    migrated
}

fn migrate_profile_hotkey(
    profiles: &mut serde_json::Map<String, serde_json::Value>,
    profile_key: &str,
    old_hotkey: &str,
    new_hotkey: &str,
) -> bool {
    let Some(profile) = profiles
        .get_mut(profile_key)
        .and_then(serde_json::Value::as_object_mut)
    else {
        return false;
    };

    if profile.get("hotkey").and_then(serde_json::Value::as_str) != Some(old_hotkey) {
        return false;
    }

    profile.insert(
        "hotkey".to_string(),
        serde_json::Value::String(new_hotkey.to_string()),
    );
    true
}

#[cfg(test)]
pub fn migrate_context_workflows_for_test(json: &mut serde_json::Value) -> bool {
    migrate_context_workflows(json)
}

fn migrate_context_workflows(json: &mut serde_json::Value) -> bool {
    let mut migrated = false;

    if json.get("workflow_profiles").is_none() {
        let profiles = json
            .get("shortcut_profiles")
            .cloned()
            .and_then(|value| serde_json::from_value::<ShortcutProfilesMap>(value).ok())
            .map(|legacy| migrate_legacy_profiles(&legacy))
            .unwrap_or_else(default_workflow_profiles);
        json["workflow_profiles"] = serde_json::to_value(profiles).unwrap_or_default();
        migrated = true;
    }

    if json.get("context_capture").is_none() {
        json["context_capture"] =
            serde_json::to_value(ContextCaptureSettings::default()).unwrap_or_default();
        migrated = true;
    }

    if json.get("application_rules").is_none() {
        json["application_rules"] = serde_json::json!([]);
        migrated = true;
    }
    migrated |= migrate_riff_into_dictate(json);
    if json.get("voice_snippets").is_none() {
        json["voice_snippets"] = serde_json::json!([]);
        migrated = true;
    }

    let ocr_enabled = json
        .get("context_capture")
        .and_then(|context| context.get("ocr_fallback"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if json
        .get("window_context_enabled")
        .and_then(serde_json::Value::as_bool)
        != Some(ocr_enabled)
    {
        json["window_context_enabled"] = serde_json::Value::Bool(ocr_enabled);
        migrated = true;
    }
    if let Some(object) = json.as_object_mut() {
        migrated |= object.remove("shortcut_profiles").is_some();
    }

    if migrated {
        tracing::info!("context_workflows_migrated");
    }
    migrated
}

fn migrate_riff_into_dictate(json: &mut serde_json::Value) -> bool {
    let mut migrated = false;

    if let Some(profiles) = json
        .get_mut("workflow_profiles")
        .and_then(serde_json::Value::as_array_mut)
    {
        let riff_template = profiles
            .iter()
            .find(|profile| profile.get("id").and_then(serde_json::Value::as_str) == Some("riff"))
            .and_then(|profile| profile.get("polish_template_id"))
            .filter(|template| !template.is_null())
            .cloned();

        if let Some(dictate) = profiles.iter_mut().find(|profile| {
            profile.get("id").and_then(serde_json::Value::as_str) == Some("dictate")
        }) {
            let template = dictate
                .get("polish_template_id")
                .filter(|template| !template.is_null())
                .cloned()
                .or(riff_template);
            if dictate.get("polish_template_id") != template.as_ref() {
                dictate["polish_template_id"] = template.clone().unwrap_or(serde_json::Value::Null);
                migrated = true;
            }
        }

        let previous_len = profiles.len();
        profiles.retain(|profile| {
            profile.get("id").and_then(serde_json::Value::as_str) != Some("riff")
        });
        migrated |= profiles.len() != previous_len;
    }

    if let Some(rules) = json
        .get_mut("application_rules")
        .and_then(serde_json::Value::as_array_mut)
    {
        for rule in rules {
            if rule.get("profile_id").and_then(serde_json::Value::as_str) == Some("riff") {
                rule["profile_id"] = serde_json::Value::String("dictate".to_string());
                migrated = true;
            }
        }
    }

    migrated
}

fn profile_trigger_mode(profile_key: &str, legacy_recording_mode: Option<&str>) -> &'static str {
    if let Some(recording_mode) = legacy_recording_mode {
        if recording_mode.eq_ignore_ascii_case("hold") {
            return "hold";
        }
        if recording_mode.eq_ignore_ascii_case("toggle") {
            return "toggle";
        }
    }

    match profile_key {
        "dictate" => "hold",
        "riff" | "custom" => "toggle",
        _ => "hold",
    }
}

pub fn load_settings_from_disk() -> AppSettings {
    let path = get_settings_path();
    if path.exists() {
        if let Ok(json) = fs::read_to_string(&path) {
            let mut json_value: serde_json::Value = match serde_json::from_str(&json) {
                Ok(v) => v,
                Err(_) => match serde_json::from_str::<AppSettings>(&json) {
                    Ok(settings) => {
                        tracing::info!(path = %path.display(), "settings_loaded");
                        return settings;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "settings_parse_failed");
                        return AppSettings::default();
                    }
                },
            };

            let migrated_cloud = migrate_cloud_settings(&mut json_value);
            let migrated_model = validate_model_name(&mut json_value);
            let migrated_profiles = migrate_to_profiles_map(&mut json_value);
            let migrated_platform_shortcuts = migrate_platform_shortcut_defaults(&mut json_value);
            let migrated_context_workflows = migrate_context_workflows(&mut json_value);
            let migrated = migrated_cloud
                || migrated_model
                || migrated_profiles
                || migrated_platform_shortcuts
                || migrated_context_workflows;

            match serde_json::from_value::<AppSettings>(json_value.clone()) {
                Ok(settings) => {
                    tracing::info!(path = %path.display(), migrated = migrated, "settings_loaded-migrated");

                    if migrated {
                        if let Ok(pretty_json) = serde_json::to_string_pretty(&settings) {
                            let _ = fs::write(&path, pretty_json);
                        }
                    }

                    return settings;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "settings_parse_failed")
                }
            }
        }
    } else {
        tracing::info!(path = %path.display(), "settings_not_found");
    }
    AppSettings::default()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock();
    Ok(settings.clone())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let previous_bridge_enabled = state.settings.lock().developer_bridge_enabled;
    let mut should_clear_cache = false;
    let mut model_to_preload: Option<String> = None;
    let mut stay_in_tray_to_apply: Option<bool> = None;
    let mut local_polish_runtime_to_apply: Option<LocalPolishRuntimeSettings> = None;
    let mut local_polish_runtime_action = LocalPolishRuntimeSettingAction::None;
    let mut retention_to_apply: Option<(RetentionPolicy, RetentionPolicy)> = None;
    let preset_to_apply: Option<String>;
    let indicator_mode_to_apply: Option<String>;
    if key == "hotkey" {
        let hotkey = value
            .as_str()
            .ok_or_else(|| "Hotkey must be a string".to_string())?;
        let mut profiles = state.settings.lock().workflow_profiles.clone();
        let dictate = profiles
            .iter_mut()
            .find(|profile| profile.id == "dictate")
            .ok_or_else(|| "Dictate profile is missing".to_string())?;
        dictate.hotkey = hotkey.to_string();
        return update_settings(
            app,
            state,
            "workflow_profiles".to_string(),
            serde_json::to_value(profiles).map_err(|error| error.to_string())?,
        );
    }
    let workflow_profile_transaction = if key == "workflow_profiles" {
        let requested = serde_json::from_value::<Vec<WorkflowProfile>>(value.clone())
            .map_err(|error| format!("Invalid workflow profiles: {error}"))?;
        crate::services::product_workflows::validate_profiles(&requested)?;
        let previous = {
            let settings = state.settings.lock();
            crate::services::product_workflows::validate_application_rules(
                &settings.application_rules,
                &requested,
            )?;
            settings.workflow_profiles.clone()
        };
        let manager = app
            .try_state::<crate::shortcut::ShortcutManager>()
            .ok_or_else(|| "Shortcut manager not available".to_string())?;
        apply_profile_registration_transaction(
            &mut ShortcutManagerRegistrar(&manager),
            &previous,
            &requested,
        )?;
        Some((previous, requested))
    } else {
        None
    };

    {
        let mut settings = state.settings.lock();

        match key.as_str() {
            "workflow_profiles" => {
                let (_, requested) = workflow_profile_transaction
                    .as_ref()
                    .ok_or_else(|| "Workflow profile transaction is missing".to_string())?;
                settings.workflow_profiles = requested.clone();
            }
            "application_rules" => {
                let rules = serde_json::from_value::<Vec<ApplicationRule>>(value.clone())
                    .map_err(|error| format!("Invalid application rules: {error}"))?;
                crate::services::product_workflows::validate_application_rules(
                    &rules,
                    &settings.workflow_profiles,
                )?;
                settings.application_rules = rules;
            }
            "voice_snippets" => {
                let snippets = serde_json::from_value::<Vec<VoiceSnippet>>(value.clone())
                    .map_err(|error| format!("Invalid voice snippets: {error}"))?;
                crate::services::product_workflows::validate_snippets(&snippets)?;
                settings.voice_snippets = snippets;
            }
            "context_capture" => {
                let context_capture =
                    serde_json::from_value::<ContextCaptureSettings>(value.clone())
                        .map_err(|error| format!("Invalid context capture settings: {error}"))?;
                settings.window_context_enabled = context_capture.ocr_fallback;
                settings.context_capture = context_capture;
            }
            "recording_mode" => {
                if let Some(v) = value.as_str() {
                    settings.recording_mode = v.to_string();
                }
            }
            "model" => {
                if let Some(v) = value.as_str() {
                    if settings.model != v {
                        should_clear_cache = true;
                        model_to_preload = Some(v.to_string());
                        if let Some(engine_type) =
                            crate::stt_engine::UnifiedEngineManager::get_engine_by_model_name(v)
                        {
                            settings.stt_engine = engine_type.to_string();
                        }
                    }
                    settings.model = v.to_string();
                }
            }
            "stt_engine" => {
                if let Some(v) = value.as_str() {
                    settings.stt_engine = v.to_string();
                }
            }
            "pill_position" => {
                if let Some(v) = value.as_str() {
                    settings.pill_position = v.to_string();
                }
            }
            "pill_indicator_mode" => {
                if let Some(v) = value.as_str() {
                    settings.pill_indicator_mode = v.to_string();
                }
            }
            "auto_start" => {
                if let Some(v) = value.as_bool() {
                    settings.auto_start = v;
                }
            }
            "gpu_acceleration" => {
                if let Some(v) = value.as_bool() {
                    if v != settings.gpu_acceleration {
                        should_clear_cache = true;
                        state.engine_manager.set_provider(v);
                    }
                    settings.gpu_acceleration = v;
                }
            }
            "language" => {
                if let Some(v) = value.as_str() {
                    settings.language = v.to_string();
                }
            }
            "stt_engine_language" => {
                if let Some(v) = value.as_str() {
                    if settings.stt_engine_language != v {
                        state.engine_manager.clear_cache();
                    }
                    settings.stt_engine_language = v.to_string();
                }
            }
            "beep_on_record" => {
                if let Some(v) = value.as_bool() {
                    if settings.beep_on_record != v {
                        if v {
                            crate::audio::beep::enable_beep();
                        } else {
                            crate::audio::beep::disable_beep();
                        }
                    }
                    settings.beep_on_record = v;
                }
            }
            "audio_device" => {
                if let Some(v) = value.as_str() {
                    settings.audio_device = v.to_string();
                }
            }
            "polish_system_prompt" => {
                if let Some(v) = value.as_str() {
                    settings.polish_system_prompt = v.to_string();
                }
            }
            "polish_model" => {
                if let Some(v) = value.as_str() {
                    settings.polish_model = v.to_string();
                }
            }
            "theme_mode" => {
                if let Some(v) = value.as_str() {
                    settings.theme_mode = v.to_string();
                }
            }
            "stt_engine_initial_prompt" => {
                if let Some(v) = value.as_str() {
                    settings.stt_engine_initial_prompt = v.to_string();
                }
            }
            "model_resident" => {
                if let Some(v) = value.as_bool() {
                    if v != settings.model_resident {
                        should_clear_cache = true;
                        if v {
                            model_to_preload = Some(settings.model.clone());
                        }
                    }
                    settings.model_resident = v;
                }
            }
            "idle_unload_minutes" => {
                if let Some(v) = value.as_u64() {
                    settings.idle_unload_minutes = v as u32;
                }
            }
            "denoise_mode" => {
                if let Some(v) = value.as_str() {
                    settings.denoise_mode = v.to_string();
                }
            }
            "stt_engine_work_domain" => {
                if let Some(v) = value.as_str() {
                    settings.stt_engine_work_domain = v.to_string();
                }
            }
            "stt_engine_work_domain_prompt" => {
                if let Some(v) = value.as_str() {
                    settings.stt_engine_work_domain_prompt = v.to_string();
                }
            }
            "stt_engine_work_subdomain" => {
                if let Some(v) = value.as_str() {
                    settings.stt_engine_work_subdomain = v.to_string();
                }
            }
            "stt_engine_user_glossary" => {
                if let Some(v) = value.as_str() {
                    settings.stt_engine_user_glossary = v.to_string();
                }
            }
            "custom_dictionary" => {
                if let Some(v) = value.as_str() {
                    settings.custom_dictionary = v.to_string();
                }
            }
            "analytics_opt_in" => {
                if let Some(v) = value.as_bool() {
                    settings.analytics_opt_in = v;
                }
            }
            "text_retention" => {
                let policy = serde_json::from_value::<RetentionPolicy>(value.clone())
                    .map_err(|e| format!("Invalid text retention policy: {e}"))?;
                settings.text_retention = policy;
                retention_to_apply = Some((settings.text_retention, settings.audio_retention));
            }
            "audio_retention" => {
                let policy = serde_json::from_value::<RetentionPolicy>(value.clone())
                    .map_err(|e| format!("Invalid audio retention policy: {e}"))?;
                settings.audio_retention = policy;
                retention_to_apply = Some((settings.text_retention, settings.audio_retention));
            }
            "vad_enabled" => {
                if let Some(v) = value.as_bool() {
                    settings.vad_enabled = v;
                }
            }
            "stay_in_tray" => {
                if let Some(v) = value.as_bool() {
                    settings.stay_in_tray = v;
                    stay_in_tray_to_apply = Some(v);
                }
            }
            "cloud_stt_enabled" => {
                if let Some(v) = value.as_bool() {
                    settings.cloud_stt_enabled = v;
                }
            }
            "active_cloud_stt_provider" => {
                if let Some(v) = value.as_str() {
                    settings.active_cloud_stt_provider = v.to_string();
                }
            }
            "cloud_stt_configs" => {
                match serde_json::from_value::<HashMap<String, CloudSttConfig>>(value.clone()) {
                    Ok(v) => {
                        settings.cloud_stt_configs = v;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, value = ?value, "cloud_stt_configs_parse_failed");
                    }
                }
            }
            "cloud_polish_enabled" => {
                if let Some(v) = value.as_bool() {
                    settings.cloud_polish_enabled = v;
                    local_polish_runtime_action =
                        polish_runtime_action_for_setting_update(&key, &value);
                }
            }
            "active_cloud_polish_provider" => {
                if let Some(v) = value.as_str() {
                    settings.active_cloud_polish_provider = v.to_string();
                }
            }
            "cloud_polish_configs" => {
                match serde_json::from_value::<HashMap<String, CloudProviderConfig>>(value.clone())
                {
                    Ok(v) => {
                        settings.cloud_polish_configs = v;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, value = ?value, "cloud_polish_configs_parse_failed");
                    }
                }
            }
            "local_polish_runtime" => {
                match serde_json::from_value::<LocalPolishRuntimeSettings>(value.clone()) {
                    Ok(v) => {
                        settings.local_polish_runtime = v.clone();
                        local_polish_runtime_to_apply = Some(v);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, value = ?value, "local_polish_runtime_parse_failed");
                    }
                }
            }

            "original_target_enabled" => {
                let enabled = serde_json::from_value::<bool>(value.clone())
                    .map_err(|error| format!("Invalid original target enablement: {error}"))?;
                settings.original_target_enabled = enabled;
            }
            "developer_bridge_enabled" => {
                settings.developer_bridge_enabled = value
                    .as_bool()
                    .ok_or_else(|| "Developer bridge setting must be a boolean".to_string())?;
            }
            "vibe_coding_enabled" => {
                settings.vibe_coding_enabled = value
                    .as_bool()
                    .ok_or_else(|| "Vibe coding setting must be a boolean".to_string())?;
            }
            "original_target_mode" => {
                let mode = serde_json::from_value::<OriginalTargetMode>(value.clone())
                    .map_err(|error| format!("Invalid original target mode: {error}"))?;
                settings.original_target_mode = mode;
            }
            "window_context_enabled" => {
                if let Some(v) = value.as_bool() {
                    settings.window_context_enabled = v;
                    settings.context_capture.ocr_fallback = v;
                }
            }
            "pill_size" => {
                if let Some(v) = value.as_u64() {
                    let size = v as u8;
                    if (1..=5).contains(&size) {
                        settings.pill_size = size;
                    }
                }
            }
            "pill_background_color" => {
                if let Some(v) = value.as_str() {
                    if let Some(color) = normalize_pill_background_color(v) {
                        settings.pill_background_color = color;
                    }
                }
            }
            "pill_background_opacity" => {
                if let Some(v) = value.as_f64() {
                    if let Some(opacity) = normalize_pill_background_opacity(v) {
                        settings.pill_background_opacity = opacity;
                    }
                }
            }
            "correction_memory_enabled" => {
                if let Some(v) = value.as_bool() {
                    settings.correction_memory_enabled = v;
                }
            }
            _ => return Err(format!("Unknown setting key: {}", key)),
        }

        let path = get_settings_path();
        let persist_result = (|| {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let json = serde_json::to_string_pretty(&*settings).map_err(|e| e.to_string())?;
            fs::write(&path, json).map_err(|e| e.to_string())
        })();
        if let Err(error) = persist_result {
            if key == "developer_bridge_enabled" {
                settings.developer_bridge_enabled = previous_bridge_enabled;
            }
            if let Some((previous, requested)) = workflow_profile_transaction.as_ref() {
                settings.workflow_profiles = previous.clone();
                if let Some(manager) = app.try_state::<crate::shortcut::ShortcutManager>() {
                    let rollback = apply_profile_registration_transaction(
                        &mut ShortcutManagerRegistrar(&manager),
                        requested,
                        previous,
                    );
                    if let Err(rollback_error) = rollback {
                        return Err(format!(
                            "{error}; workflow profile rollback failed: {rollback_error}"
                        ));
                    }
                }
            }
            return Err(error);
        }

        preset_to_apply = if key == "pill_position" {
            Some(settings.pill_position.clone())
        } else {
            None
        };

        // Check if pill_indicator_mode changed
        indicator_mode_to_apply = if key == "pill_indicator_mode" {
            Some(settings.pill_indicator_mode.clone())
        } else {
            None
        };

        info!(key = %key, "settings_updated");
        if key != "developer_bridge_enabled" {
            let _ = app.emit(EventName::SETTINGS_CHANGED, settings.clone());
        }
    } // lock released here

    if let Some(runtime_settings) = local_polish_runtime_to_apply {
        if let Err(e) = state
            .polish_manager
            .configure_local_runtime(&runtime_settings)
        {
            tracing::warn!(error = %e, "local_polish_runtime_configure_failed-settings_update");
        }
    }

    if local_polish_runtime_action == LocalPolishRuntimeSettingAction::StopManagedRuntime {
        state.polish_manager.stop_local_runtime();
    }

    if let Some((text_policy, audio_policy)) = retention_to_apply {
        let store = state.history_store.lock();
        let report = store.cleanup_retention(text_policy, audio_policy)?;
        info!(
            text_entries_deleted = report.text_entries_deleted,
            audio_files_deleted = report.audio_files_deleted,
            missing_audio_references_cleared = report.missing_audio_references_cleared,
            "retention_policy_applied"
        );
    }

    if let Some(preset) = preset_to_apply {
        position_pill_window(&app, &preset);
    }

    if key == "developer_bridge_enabled" {
        if let Err(error) = crate::commands::platform_quality::sync_developer_bridge(app.clone()) {
            state.settings.lock().developer_bridge_enabled = previous_bridge_enabled;
            let rollback = save_settings_internal(&app);
            let _ = app.emit(EventName::SETTINGS_CHANGED, state.settings.lock().clone());
            return Err(match rollback {
                Ok(()) => error,
                Err(rollback_error) => {
                    format!("{error}; failed to restore bridge setting: {rollback_error}")
                }
            });
        }
        let _ = app.emit(EventName::SETTINGS_CHANGED, state.settings.lock().clone());
    }

    if indicator_mode_to_apply.is_some() {
        crate::commands::window::update_pill_visibility(&app);
    }

    if let Some(stay_in_tray) = stay_in_tray_to_apply {
        if stay_in_tray {
            if let Err(e) = crate::tray::show_tray(&app) {
                tracing::error!(error = %e, "tray_show_failed");
            }
        } else {
            crate::tray::remove_tray(&app);
        }
    }

    if should_clear_cache {
        state.engine_manager.clear_cache();
    }

    if let Some(model_name) = model_to_preload {
        let engine_type =
            crate::stt_engine::UnifiedEngineManager::get_engine_by_model_name(&model_name)
                .unwrap_or(crate::stt_engine::traits::EngineType::Whisper);
        let engine_manager = state.engine_manager.clone();
        let app_clone = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = engine_manager.load_model(engine_type, &model_name) {
                tracing::warn!(model = %model_name, error = %e, "model_preload_failed");
            } else {
                tracing::info!(model = %model_name, mem_mb = get_process_rss_mb(), "model_preloaded");
                let _ = app_clone.emit(
                    EventName::MODEL_LOADED,
                    crate::events::ModelLoadedEvent { model: model_name },
                );
            }
        });
    }

    Ok(())
}

/// Returns the current process RSS memory in MB, or 0 if unavailable.
fn get_process_rss_mb() -> u64 {
    let pid = std::process::id();
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

fn get_subdomains_for_domain(domain: &str) -> Vec<String> {
    match domain {
        "it" => vec![
            "general".to_string(),
            "security".to_string(),
            "hardware".to_string(),
            "software".to_string(),
            "web".to_string(),
            "ai".to_string(),
        ],
        "legal" => vec![
            "general".to_string(),
            "civil".to_string(),
            "criminal".to_string(),
            "corporate".to_string(),
            "international".to_string(),
        ],
        "medical" => vec![
            "general".to_string(),
            "pharmacy".to_string(),
            "diagnostics".to_string(),
            "cardiology".to_string(),
            "neurology".to_string(),
        ],
        _ => vec![],
    }
}

#[tauri::command]
pub fn get_glossary_content(_subdomain: String) -> Result<String, String> {
    // User maintains their own glossary - no default content
    Ok(String::new())
}

#[tauri::command]
pub fn get_available_subdomains(domain: String) -> Result<Vec<String>, String> {
    Ok(get_subdomains_for_domain(&domain))
}

#[tauri::command]
pub fn get_cloud_provider_schemas() -> crate::provider_schema::CloudProviderSchemas {
    crate::provider_schema::get_schemas()
}

#[tauri::command]
pub async fn check_active_cloud_stt_config(
    state: State<'_, AppState>,
) -> Result<CloudConnectionCheckResult, String> {
    let started_at = Instant::now();
    let (enabled, config, language) = {
        let settings = state.settings.lock();
        (
            settings.cloud_stt_enabled,
            settings.get_active_cloud_stt_config(),
            settings.stt_engine_language.clone(),
        )
    };

    if !enabled {
        return Ok(CloudConnectionCheckResult::failure(
            "disabled",
            "Cloud STT is disabled.",
            elapsed_ms(started_at),
        ));
    }

    if let Err(err) = validate_cloud_stt_config_for_check(&config) {
        return Ok(CloudConnectionCheckResult::failure(
            err.kind,
            err.message,
            elapsed_ms(started_at),
        ));
    }

    let mut client = match crate::stt_engine::cloud::StreamingSttClient::new(
        config.clone(),
        Some(language.as_str()),
        crate::stt_engine::traits::SttContext::default(),
    ) {
        Ok(client) => client,
        Err(error) => {
            let kind = classify_cloud_check_error(&error);
            return Ok(CloudConnectionCheckResult::failure(
                kind,
                cloud_check_user_message(kind),
                elapsed_ms(started_at),
            ));
        }
    };

    let result = tokio::time::timeout(CLOUD_CONFIG_CHECK_TIMEOUT, client.connect()).await;
    let check_result = match result {
        Ok(Ok(())) => {
            client.close().await;
            info!(
                provider = %config.provider_type,
                duration_ms = elapsed_ms(started_at),
                "cloud_stt_config_check_ok"
            );
            CloudConnectionCheckResult::success(
                "Connection OK. Endpoint and credentials are usable.",
                elapsed_ms(started_at),
            )
        }
        Ok(Err(error)) => {
            client.close().await;
            let kind = classify_cloud_check_error(&error);
            warn!(
                provider = %config.provider_type,
                kind,
                error = %error,
                "cloud_stt_config_check_failed"
            );
            CloudConnectionCheckResult::failure(
                kind,
                cloud_check_user_message(kind),
                elapsed_ms(started_at),
            )
        }
        Err(_) => {
            client.close().await;
            warn!(
                provider = %config.provider_type,
                timeout_secs = CLOUD_CONFIG_CHECK_TIMEOUT.as_secs(),
                "cloud_stt_config_check_timeout"
            );
            CloudConnectionCheckResult::failure(
                "timeout",
                cloud_check_user_message("timeout"),
                elapsed_ms(started_at),
            )
        }
    };

    Ok(check_result)
}

#[tauri::command]
pub async fn check_active_cloud_polish_config(
    state: State<'_, AppState>,
) -> Result<CloudConnectionCheckResult, String> {
    let started_at = Instant::now();
    let (enabled, provider_type, mut config) = {
        let settings = state.settings.lock();
        let provider_type = settings.active_cloud_polish_provider.clone();
        let config = settings
            .cloud_polish_configs
            .get(&provider_type)
            .cloned()
            .unwrap_or_default();
        (settings.cloud_polish_enabled, provider_type, config)
    };

    config.enabled = enabled;
    config.provider_type = provider_type.clone();

    if !enabled {
        return Ok(CloudConnectionCheckResult::failure(
            "disabled",
            "Cloud Polish is disabled.",
            elapsed_ms(started_at),
        ));
    }

    if let Err(err) = validate_cloud_polish_config_for_check(&config) {
        return Ok(CloudConnectionCheckResult::failure(
            err.kind,
            err.message,
            elapsed_ms(started_at),
        ));
    }

    let engine = crate::polish_engine::cloud::CloudPolishEngine::new(
        crate::polish_engine::cloud::CloudProviderConfig {
            provider_type: config.provider_type.clone(),
            api_key: config.api_key.clone(),
            base_url: config.base_url.clone(),
            model: config.model.clone(),
            enable_thinking: config.enable_thinking,
        },
    );

    match engine.check_connection().await {
        Ok(()) => {
            info!(
                provider = %config.provider_type,
                model = %config.model,
                duration_ms = elapsed_ms(started_at),
                "cloud_polish_config_check_ok"
            );
            Ok(CloudConnectionCheckResult::success(
                "Connection OK. Endpoint, credentials, and model are usable.",
                elapsed_ms(started_at),
            ))
        }
        Err(error) => {
            let kind = classify_cloud_check_error(&error);
            warn!(
                provider = %config.provider_type,
                model = %config.model,
                kind,
                error = %error,
                "cloud_polish_config_check_failed"
            );
            Ok(CloudConnectionCheckResult::failure(
                kind,
                cloud_check_user_message(kind),
                elapsed_ms(started_at),
            ))
        }
    }
}

#[tauri::command]
pub async fn check_local_polish_runtime_config(
    state: State<'_, AppState>,
) -> Result<CloudConnectionCheckResult, String> {
    let started_at = Instant::now();
    let runtime_settings = {
        let settings = state.settings.lock();
        settings.local_polish_runtime.clone()
    };

    if !validate_required_url(&runtime_settings.base_url) {
        return Ok(CloudConnectionCheckResult::failure(
            "invalid_url",
            "Invalid local polish runtime URL.",
            elapsed_ms(started_at),
        ));
    }

    let polish_manager = state.polish_manager.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        polish_manager
            .check_local_runtime_config(&runtime_settings, LOCAL_POLISH_RUNTIME_CHECK_TIMEOUT)
    })
    .await
    .map_err(|e| format!("Local polish runtime check task failed: {e}"))?;

    match result {
        Ok(()) => {
            info!(
                duration_ms = elapsed_ms(started_at),
                "local_polish_runtime_config_check_ok"
            );
            Ok(CloudConnectionCheckResult::success(
                "Local polish runtime is reachable.",
                elapsed_ms(started_at),
            ))
        }
        Err(error) => {
            let kind = classify_cloud_check_error(&error);
            warn!(
                kind,
                error = %error,
                "local_polish_runtime_config_check_failed"
            );
            Ok(CloudConnectionCheckResult::failure(
                kind,
                cloud_check_user_message(kind),
                elapsed_ms(started_at),
            ))
        }
    }
}

fn polish_runtime_action_for_setting_update(
    key: &str,
    value: &serde_json::Value,
) -> LocalPolishRuntimeSettingAction {
    if key == "cloud_polish_enabled" && value.as_bool() == Some(true) {
        return LocalPolishRuntimeSettingAction::StopManagedRuntime;
    }

    LocalPolishRuntimeSettingAction::None
}

/// Scans the models directory for legacy model files (ggml/gguf format)
/// and deletes them. These are from the old whisper.cpp format that is no longer used.
/// Current models use sherpa-onnx ONNX format (.onnx, .int8.onnx).
///
/// Returns the number of legacy files deleted.
pub fn cleanup_legacy_models() -> Result<usize, String> {
    let models_dir = AppPaths::models_dir();

    if !models_dir.exists() {
        info!(path = ?models_dir, "cleanup_legacy_models_skip-no_models_dir");
        return Ok(0);
    }

    let mut deleted_count = 0;
    let legacy_extensions = [".ggml", ".gguf"];

    let entries = fs::read_dir(&models_dir).map_err(|e| {
        format!(
            "Failed to read models directory '{}': {}",
            models_dir.display(),
            e
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Check if it's a file with a legacy extension
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if legacy_extensions
                    .iter()
                    .any(|&e| e == format!(".{}", ext_lower))
                {
                    match fs::remove_file(&path) {
                        Ok(_) => {
                            info!(file = %path.display(), "legacy_model_file_deleted");
                            deleted_count += 1;
                        }
                        Err(e) => {
                            warn!(file = %path.display(), error = %e, "legacy_model_file_deletion_failed");
                        }
                    }
                }
            }
        }

        // Also check for legacy model subdirectories (e.g., "model.ggml" folders)
        if path.is_dir() {
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            if dir_name.ends_with(".ggml") || dir_name.ends_with(".gguf") {
                match fs::remove_dir_all(&path) {
                    Ok(_) => {
                        info!(dir = %path.display(), "legacy_model_dir_deleted");
                        deleted_count += 1;
                    }
                    Err(e) => {
                        warn!(dir = %path.display(), error = %e, "legacy_model_dir_deletion_failed");
                    }
                }
            }
        }
    }

    info!(deleted = deleted_count, "cleanup_legacy_models_complete");
    Ok(deleted_count)
}

#[tauri::command]
pub async fn cleanup_legacy_models_cmd() -> Result<usize, String> {
    cleanup_legacy_models()
}

#[cfg(test)]
mod __test__;
