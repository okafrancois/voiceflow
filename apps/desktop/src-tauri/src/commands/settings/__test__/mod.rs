use super::{
    classify_cloud_check_error, migrate_context_workflows_for_test,
    migrate_platform_shortcut_defaults_for_test, migrate_to_profiles_map_for_test,
    normalize_pill_background_color, normalize_pill_background_opacity,
    polish_runtime_action_for_setting_update, validate_cloud_polish_config_for_check,
    validate_cloud_stt_config_for_check, AppSettings, CloudProviderConfig, CloudSttConfig,
    LocalPolishRuntimeSettingAction, OriginalTargetMode,
};
use crate::history::RetentionPolicy;
use serde_json::json;

#[test]
fn test_is_streaming_stt_active_accepts_aliyun_stream_provider_id() {
    let mut settings = AppSettings::default();
    settings.cloud_stt_enabled = true;
    settings.active_cloud_stt_provider = "aliyun-stream".to_string();

    assert!(settings.is_streaming_stt_active());
}

#[test]
fn cloud_stt_check_validation_requires_schema_fields() {
    let mut config = CloudSttConfig {
        provider_type: "volcengine-streaming".to_string(),
        api_key: "token".to_string(),
        app_id: String::new(),
        base_url: String::new(),
        model: String::new(),
        language: String::new(),
        enabled: true,
    };

    let err = validate_cloud_stt_config_for_check(&config).unwrap_err();
    assert_eq!(err.kind, "missing_required");
    assert!(err.message.contains("App ID"));

    config.app_id = "app-id".to_string();
    assert!(validate_cloud_stt_config_for_check(&config).is_ok());
}

#[test]
fn cloud_polish_check_validation_requires_model() {
    let config = CloudProviderConfig {
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        base_url: String::new(),
        model: String::new(),
        enable_thinking: false,
        enabled: true,
    };

    let err = validate_cloud_polish_config_for_check(&config).unwrap_err();
    assert_eq!(err.kind, "missing_required");
    assert!(err.message.contains("Model"));
}

#[test]
fn cloud_check_validation_rejects_invalid_base_url() {
    let config = CloudProviderConfig {
        provider_type: "anthropic".to_string(),
        api_key: "sk-test".to_string(),
        base_url: "not a url".to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        enable_thinking: false,
        enabled: true,
    };

    let err = validate_cloud_polish_config_for_check(&config).unwrap_err();
    assert_eq!(err.kind, "invalid_url");
}

#[test]
fn cloud_check_error_classifier_maps_auth_and_timeout() {
    assert_eq!(
        classify_cloud_check_error("API error (401 Unauthorized): invalid_api_key"),
        "auth_failed"
    );
    assert_eq!(
        classify_cloud_check_error("connection check timed out after 10s"),
        "timeout"
    );
    assert_eq!(
        classify_cloud_check_error("API error (404): model not found"),
        "model_failed"
    );
}

#[test]
fn enabling_cloud_polish_stops_managed_local_runtime() {
    assert_eq!(
        polish_runtime_action_for_setting_update("cloud_polish_enabled", &json!(true)),
        LocalPolishRuntimeSettingAction::StopManagedRuntime
    );
}

#[test]
fn disabling_cloud_polish_keeps_local_runtime_available() {
    assert_eq!(
        polish_runtime_action_for_setting_update("cloud_polish_enabled", &json!(false)),
        LocalPolishRuntimeSettingAction::None
    );
}

#[test]
fn migrate_from_legacy_hotkey_copies_global_recording_mode_into_profiles() {
    let mut json = json!({
        "hotkey": "Shift+Space",
        "recording_mode": "toggle",
    });

    migrate_to_profiles_map_for_test(&mut json);

    assert_eq!(
        json["shortcut_profiles"]["dictate"]["trigger_mode"],
        "toggle"
    );
    assert_eq!(json["shortcut_profiles"]["riff"]["trigger_mode"], "toggle");
}

#[test]
fn migrate_array_profiles_copies_global_recording_mode_into_existing_profiles() {
    let mut json = json!({
        "recording_mode": "hold",
        "shortcut_profiles": [
            {
                "hotkey": "Cmd+Slash",
                "action": { "Record": { "polish_template_id": null } }
            },
            {
                "hotkey": "Opt+Slash",
                "action": { "Record": { "polish_template_id": "filler" } }
            },
            {
                "hotkey": "Cmd+Alt+Slash",
                "action": { "Record": { "polish_template_id": "formal" } }
            }
        ]
    });

    migrate_to_profiles_map_for_test(&mut json);

    assert_eq!(json["shortcut_profiles"]["dictate"]["trigger_mode"], "hold");
    assert_eq!(json["shortcut_profiles"]["riff"]["trigger_mode"], "hold");
    assert_eq!(json["shortcut_profiles"]["custom"]["trigger_mode"], "hold");
}

#[test]
fn migrate_platform_shortcut_defaults_rewrites_untouched_mac_defaults_on_windows() {
    let mut json = json!({
        "shortcut_profiles": {
            "dictate": {
                "hotkey": "Cmd+Slash",
                "trigger_mode": "hold",
                "action": { "Record": { "polish_template_id": null } }
            },
            "riff": {
                "hotkey": "Opt+Slash",
                "trigger_mode": "toggle",
                "action": { "Record": { "polish_template_id": "filler" } }
            },
            "custom": {
                "hotkey": "Cmd+Alt+Slash",
                "trigger_mode": "toggle",
                "action": { "Record": { "polish_template_id": null } }
            }
        }
    });

    let migrated = migrate_platform_shortcut_defaults_for_test(&mut json, false);

    assert!(migrated);
    assert_eq!(json["shortcut_profiles"]["dictate"]["hotkey"], "Ctrl+Slash");
    assert_eq!(json["shortcut_profiles"]["riff"]["hotkey"], "Alt+Slash");
    assert_eq!(
        json["shortcut_profiles"]["custom"]["hotkey"],
        "Cmd+Alt+Slash"
    );
}

#[test]
fn migrate_platform_shortcut_defaults_keeps_macos_and_customized_values() {
    let mut mac_json = json!({
        "shortcut_profiles": {
            "dictate": { "hotkey": "Cmd+Slash" },
            "riff": { "hotkey": "Opt+Slash" }
        }
    });
    let mut customized_json = json!({
        "shortcut_profiles": {
            "dictate": { "hotkey": "Shift+Space" },
            "riff": { "hotkey": "Ctrl+Space" }
        }
    });

    assert!(!migrate_platform_shortcut_defaults_for_test(
        &mut mac_json,
        true
    ));
    assert_eq!(
        mac_json["shortcut_profiles"]["dictate"]["hotkey"],
        "Cmd+Slash"
    );
    assert_eq!(mac_json["shortcut_profiles"]["riff"]["hotkey"], "Opt+Slash");

    assert!(!migrate_platform_shortcut_defaults_for_test(
        &mut customized_json,
        false
    ));
    assert_eq!(
        customized_json["shortcut_profiles"]["dictate"]["hotkey"],
        "Shift+Space"
    );
    assert_eq!(
        customized_json["shortcut_profiles"]["riff"]["hotkey"],
        "Ctrl+Space"
    );
}

#[test]
fn context_workflow_migration_preserves_legacy_profile_assignments() {
    let mut json = json!({
        "shortcut_profiles": {
            "dictate": {
                "hotkey": "Cmd+D",
                "trigger_mode": "hold",
                "action": { "Record": { "polish_template_id": null } }
            },
            "riff": {
                "hotkey": "Cmd+R",
                "trigger_mode": "toggle",
                "action": { "Record": { "polish_template_id": "filler" } }
            },
            "custom": {
                "hotkey": "Cmd+C",
                "trigger_mode": "double_tap",
                "action": { "Record": { "polish_template_id": "formal" } }
            }
        },
        "window_context_enabled": true
    });

    assert!(migrate_context_workflows_for_test(&mut json));
    assert_eq!(json["workflow_profiles"].as_array().unwrap().len(), 3);
    assert_eq!(json["workflow_profiles"][0]["hotkey"], "Cmd+D");
    assert_eq!(json["workflow_profiles"][2]["hotkey"], "Cmd+C");
    assert_eq!(json["workflow_profiles"][2]["polish_template_id"], "formal");
    assert_eq!(json["context_capture"]["clipboard"], false);
    assert_eq!(json["context_capture"]["ocr_fallback"], false);
    assert_eq!(json["window_context_enabled"], false);
}

#[test]
fn workflow_settings_default_to_structured_context_without_sensitive_fallbacks() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert_eq!(settings.workflow_profiles.len(), 2);
    assert!(settings.workflow_profiles[0].protected);
    assert!(settings.context_capture.application_metadata);
    assert!(settings.context_capture.selected_text);
    assert!(!settings.context_capture.clipboard);
    assert!(!settings.context_capture.ocr_fallback);
    assert!(!settings.window_context_enabled);
}

#[test]
fn missing_pill_background_color_uses_default() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert_eq!(settings.pill_background_color, "#1d1d1d");
    assert_eq!(settings.pill_background_opacity, 1.0);
}

#[test]
fn correction_memory_defaults_enabled() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert!(settings.correction_memory_enabled);
}

#[test]
fn stay_in_tray_defaults_enabled() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert!(settings.stay_in_tray);
}

#[test]
fn local_polish_runtime_uses_default_when_missing() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert_eq!(settings.local_polish_runtime.provider_type, "llama-server");
    assert_eq!(
        settings.local_polish_runtime.base_url,
        "http://127.0.0.1:8000/v1"
    );
    assert_eq!(settings.local_polish_runtime.ready_timeout_secs, 20);
}

#[test]
fn direct_stream_typing_defaults_disabled() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert!(!settings.polish_stream_direct_typing_enabled);
}

#[test]
fn original_target_delivery_defaults_disabled_with_foreground_preference() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert!(!settings.original_target_enabled);
    assert_eq!(
        settings.original_target_mode,
        OriginalTargetMode::Foreground
    );
}

#[test]
fn original_target_mode_rejects_unknown_values() {
    let result = serde_json::from_value::<AppSettings>(json!({
        "original_target_mode": "teleport"
    }));

    assert!(result.is_err());
}

#[test]
fn retention_defaults_preserve_text_for_ninety_days_and_delete_audio() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert_eq!(settings.text_retention, RetentionPolicy::Days90);
    assert_eq!(settings.audio_retention, RetentionPolicy::Never);
}

#[test]
fn retention_settings_reject_unknown_policy_values() {
    let result = serde_json::from_value::<AppSettings>(json!({
        "text_retention": "sometimes"
    }));

    assert!(result.is_err());
}

#[test]
fn normalize_pill_background_color_accepts_only_hex_rgb_values() {
    assert_eq!(
        normalize_pill_background_color(" #AABBCC "),
        Some("#aabbcc".to_string())
    );
    assert_eq!(normalize_pill_background_color("#abc"), None);
    assert_eq!(normalize_pill_background_color("red"), None);
    assert_eq!(normalize_pill_background_color("#zzzzzz"), None);
}

#[test]
fn normalize_pill_background_opacity_clamps_to_visible_range() {
    assert_eq!(normalize_pill_background_opacity(0.65), Some(0.65));
    assert_eq!(normalize_pill_background_opacity(0.0), Some(0.2));
    assert_eq!(normalize_pill_background_opacity(1.5), Some(1.0));
    assert_eq!(normalize_pill_background_opacity(f64::NAN), None);
}
