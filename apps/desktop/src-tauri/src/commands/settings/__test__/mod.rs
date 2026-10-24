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
fn apple_selection_rejects_unready_provider_without_mutating_settings() {
    use crate::stt_engine::apple::AppleSpeechStatus;
    for status in [
        None,
        Some(AppleSpeechStatus::OsUnsupported),
        Some(AppleSpeechStatus::AssetsMissing),
        Some(AppleSpeechStatus::LocaleUnsupported),
    ] {
        let mut settings = AppSettings::default();
        settings.cloud_stt_enabled = true;
        let before = serde_json::to_value(&settings).unwrap();
        let result = super::set_local_stt_model(&mut settings, "apple-speech", status);
        assert!(result.is_err(), "status {status:?} must reject selection");
        assert_eq!(serde_json::to_value(settings).unwrap(), before);
    }
}

#[test]
fn apple_selection_derives_engine_and_disables_cloud_only_when_ready() {
    use crate::stt_engine::apple::AppleSpeechStatus;
    let mut settings = AppSettings::default();
    settings.cloud_stt_enabled = true;
    settings.cloud_polish_enabled = true;
    let mut expected = serde_json::to_value(&settings).unwrap();
    expected["model"] = json!("apple-speech");
    expected["stt_engine"] = json!("apple");
    expected["cloud_stt_enabled"] = json!(false);
    super::set_local_stt_model(
        &mut settings,
        "apple-speech",
        Some(AppleSpeechStatus::Ready),
    )
    .unwrap();
    assert_eq!(serde_json::to_value(settings).unwrap(), expected);
}

#[test]
fn platform_policy_migrates_legacy_polish_idempotently_without_changing_other_settings() {
    for model_id in [
        "qwen3.5-0.8b",
        "qwen3.5-2b",
        "qwen3-4b",
        "lfm2.5-1.2b",
        "lfm2-2.6b",
        "gemma-2b-it",
        "gemma-4-e2b",
        "glm-4.7-flash-reap-23b-a3b",
    ] {
        let mut settings = json!({
            "polish_model": model_id,
            "cloud_polish_enabled": true,
            "active_cloud_polish_provider": "openai",
            "cloud_polish_configs": {"openai": {"api_key": "test-only", "model": "vendor-model"}},
            "polish_system_prompt": "Keep the custom template",
            "language": "fr",
            "model": "apple-speech",
            "local_polish_runtime": {"provider_type": "llama_cpp", "base_url": "http://127.0.0.1:8080"}
        });
        let mut expected = settings.clone();
        expected["polish_model"] = json!("mlx-qwen3-1.7b");
        assert!(
            super::migrate_local_polish_for_platform(&mut settings, true),
            "{model_id}"
        );
        assert_eq!(settings, expected);
        assert!(!super::migrate_local_polish_for_platform(
            &mut settings,
            true
        ));
        assert_eq!(settings, expected);
    }
}

#[test]
fn platform_policy_preserves_valid_polish_and_non_arm_settings() {
    for (model_id, apple_silicon) in [
        ("apple-intelligence", true),
        ("mlx-qwen3-0.6b", true),
        ("mlx-qwen3-1.7b", true),
        ("", true),
        ("unknown-model", true),
        ("qwen3.5-0.8b", false),
        ("gemma-4-e2b", false),
    ] {
        let mut settings = json!({"polish_model": model_id, "cloud_polish_enabled": false});
        let original = settings.clone();
        assert!(!super::migrate_local_polish_for_platform(
            &mut settings,
            apple_silicon
        ));
        assert_eq!(settings, original);
    }
    let mut settings = json!({"cloud_polish_enabled": true});
    let original = settings.clone();
    assert!(!super::migrate_local_polish_for_platform(
        &mut settings,
        true
    ));
    assert_eq!(settings, original);
}

#[test]
fn platform_policy_rejects_excluded_polish_selection_before_mutation() {
    let mut settings = AppSettings::default();
    settings.polish_model = "apple-intelligence".to_string();
    settings.cloud_polish_enabled = true;
    let original = serde_json::to_value(&settings).unwrap();
    for model_id in [
        "qwen3.5-0.8b",
        "lfm2.5-1.2b",
        "gemma-4-e2b",
        "glm-4.7-flash-reap-23b-a3b",
    ] {
        let result = super::set_local_polish_model(&mut settings, model_id);
        if crate::polish_engine::mlx::runtime::supported() {
            assert!(result
                .unwrap_err()
                .contains("not available on this computer"));
            assert_eq!(serde_json::to_value(&settings).unwrap(), original);
        } else {
            result.unwrap();
            assert_eq!(settings.polish_model, model_id);
            assert!(settings.cloud_polish_enabled);
        }
    }
}

#[test]
fn platform_policy_accepts_apple_and_mlx_without_changing_cloud_routing() {
    let mut settings = AppSettings::default();
    settings.cloud_polish_enabled = true;
    for model_id in ["apple-intelligence", "mlx-qwen3-0.6b", "mlx-qwen3-1.7b", ""] {
        if model_id.starts_with("mlx-") && !crate::polish_engine::mlx::runtime::supported() {
            continue;
        }
        let mut expected = serde_json::to_value(&settings).unwrap();
        expected["polish_model"] = json!(model_id);
        super::set_local_polish_model(&mut settings, model_id).unwrap();
        assert_eq!(serde_json::to_value(&settings).unwrap(), expected);
    }
}

#[test]
fn test_is_streaming_stt_active_accepts_aliyun_stream_provider_id() {
    let mut settings = AppSettings::default();
    settings.cloud_stt_enabled = true;
    settings.active_cloud_stt_provider = "aliyun-stream".to_string();

    assert!(settings.is_streaming_stt_active());
}

#[test]
fn mlx_audio_is_an_active_recording_provider_with_local_defaults() {
    let mut settings = AppSettings::default();
    settings.cloud_stt_enabled = true;
    settings.active_cloud_stt_provider = "mlx-audio".to_string();

    assert_eq!(
        settings.is_streaming_stt_active(),
        !crate::stt_engine::mlx::runtime::supported()
    );
    let config = settings.get_active_cloud_stt_config();
    assert_eq!(config.base_url, "http://127.0.0.1:8000/v1");
    assert_eq!(
        config.model,
        "mlx-community/whisper-large-v3-turbo-asr-fp16"
    );
    assert!(config.api_key.is_empty());
    assert!(validate_cloud_stt_config_for_check(&config).is_ok());
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
    assert_eq!(json["workflow_profiles"].as_array().unwrap().len(), 2);
    assert_eq!(json["workflow_profiles"][0]["hotkey"], "Cmd+D");
    assert_eq!(json["workflow_profiles"][0]["polish_template_id"], "filler");
    assert_eq!(json["workflow_profiles"][1]["hotkey"], "Cmd+C");
    assert_eq!(json["workflow_profiles"][1]["polish_template_id"], "formal");
    assert_eq!(json["context_capture"]["clipboard"], false);
    assert_eq!(json["context_capture"]["ocr_fallback"], false);
    assert_eq!(json["window_context_enabled"], false);
}

#[test]
fn workflow_settings_default_to_structured_context_without_sensitive_fallbacks() {
    let settings: AppSettings = serde_json::from_value(json!({})).unwrap();

    assert_eq!(settings.workflow_profiles.len(), 1);
    assert!(settings.workflow_profiles[0].protected);
    assert!(settings.context_capture.application_metadata);
    assert!(settings.context_capture.selected_text);
    assert!(!settings.context_capture.clipboard);
    assert!(!settings.context_capture.ocr_fallback);
    assert!(!settings.window_context_enabled);
}

#[test]
fn context_workflow_migration_removes_existing_riff_and_retargets_rules() {
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
                "action": { "Record": { "polish_template_id": "agent" } }
            }
        },
        "workflow_profiles": [
            {
                "id": "dictate",
                "name": "Dictate",
                "hotkey": "Cmd+D",
                "trigger_mode": "hold",
                "language": null,
                "polish_template_id": null,
                "translation_target": null,
                "output_action": "insert",
                "code_aware": false,
                "protected": true
            },
            {
                "id": "riff",
                "name": "Riff",
                "hotkey": "Cmd+R",
                "trigger_mode": "toggle",
                "language": null,
                "polish_template_id": "agent",
                "translation_target": null,
                "output_action": "insert",
                "code_aware": false,
                "protected": false
            }
        ],
        "application_rules": [{
            "id": "editor",
            "application_id": "com.example.Editor",
            "title_contains": null,
            "profile_id": "riff",
            "enabled": true
        }]
    });

    assert!(migrate_context_workflows_for_test(&mut json));
    assert_eq!(json["workflow_profiles"].as_array().unwrap().len(), 1);
    assert_eq!(json["workflow_profiles"][0]["id"], "dictate");
    assert_eq!(json["workflow_profiles"][0]["polish_template_id"], "agent");
    assert_eq!(json["application_rules"][0]["profile_id"], "dictate");
    assert!(json.get("shortcut_profiles").is_none());
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
fn retired_streaming_setting_is_ignored_when_loading_old_config() {
    let settings: AppSettings =
        serde_json::from_value(json!({"polish_stream_direct_typing_enabled": true})).unwrap();
    assert!(serde_json::to_value(settings)
        .unwrap()
        .get("polish_stream_direct_typing_enabled")
        .is_none());
}

#[test]
fn canonical_profiles_are_the_only_persisted_shortcut_configuration() {
    let settings = AppSettings::default();
    let serialized = serde_json::to_value(settings).unwrap();
    assert!(serialized.get("shortcut_profiles").is_none());
    assert_eq!(serialized["workflow_profiles"][0]["id"], "dictate");
}

#[test]
fn developer_bridge_is_opt_in_for_new_and_existing_configs() {
    assert!(!AppSettings::default().developer_bridge_enabled);
    let old: AppSettings = serde_json::from_value(json!({"model": "whisper-base"})).unwrap();
    assert!(!old.developer_bridge_enabled);
}

#[test]
fn vibe_coding_is_opt_in_for_new_and_existing_configs() {
    assert!(!AppSettings::default().vibe_coding_enabled);
    let old: AppSettings = serde_json::from_value(json!({"model": "whisper-base"})).unwrap();
    assert!(!old.vibe_coding_enabled);
}

#[test]
fn profile_migration_discards_legacy_map_and_is_idempotent() {
    let mut value = json!({"shortcut_profiles": {"dictate": {"hotkey": "Cmd+D", "trigger_mode": "toggle", "action": {"Record": {"polish_template_id": "formal"}}}}});
    assert!(migrate_context_workflows_for_test(&mut value));
    assert!(value.get("shortcut_profiles").is_none());
    assert_eq!(value["workflow_profiles"][0]["hotkey"], "Cmd+D");
    assert_eq!(
        value["workflow_profiles"][0]["polish_template_id"],
        "formal"
    );
    assert!(!migrate_context_workflows_for_test(&mut value));
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
