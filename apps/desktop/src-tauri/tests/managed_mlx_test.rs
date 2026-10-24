use serde_json::json;
use voiceflow_lib::commands::settings::migrate_local_stt_for_platform_for_test;
use voiceflow_lib::stt_engine::{models, EngineType};

#[test]
fn mlx_is_a_local_engine_not_a_cloud_provider() {
    assert_eq!("mlx".parse::<EngineType>().unwrap().as_str(), "mlx");
    assert_ne!("mlx".parse::<EngineType>().unwrap(), EngineType::Cloud);
}

#[test]
fn apple_silicon_offers_multiple_mlx_models_and_built_in_apple_speech() {
    let catalogue = models::for_platform(true);
    assert_eq!(catalogue.len(), 4);
    assert!(catalogue
        .iter()
        .all(|model| matches!(model.engine_type, EngineType::Mlx | EngineType::Apple)));
    assert!(catalogue
        .iter()
        .any(|model| model.engine_type == EngineType::Apple));
    for name in ["mlx-whisper-turbo", "mlx-parakeet-v3", "mlx-qwen3-asr-0.6b"] {
        assert!(catalogue.iter().any(|model| model.name == name));
    }
}

#[test]
fn intel_and_windows_keep_their_compatible_native_models() {
    let catalogue = models::for_platform(false);
    assert!(catalogue.iter().any(|model| model.name == "whisper-base"));
    assert!(catalogue
        .iter()
        .all(|model| model.engine_type.as_str() != "mlx"));
    assert_eq!(
        models::default_for_platform("fr", false).name,
        "whisper-base"
    );
    assert_eq!(
        models::default_for_platform("fr", true).name,
        "mlx-whisper-turbo"
    );
}

#[test]
fn legacy_apple_silicon_selection_migrates_without_changing_vendor_cloud_services() {
    let mut settings = json!({
        "model": "whisper-base",
        "cloud_stt_enabled": true,
        "active_cloud_stt_provider": "elevenlabs",
        "cloud_stt_configs": {"elevenlabs": {"model": "scribe_v2_realtime"}}
    });
    assert!(migrate_local_stt_for_platform_for_test(&mut settings, true));
    assert_eq!(settings["model"], "mlx-whisper-turbo");
    assert_eq!(settings["cloud_stt_enabled"], true);
    assert_eq!(settings["active_cloud_stt_provider"], "elevenlabs");
    assert_eq!(
        settings["cloud_stt_configs"]["elevenlabs"]["model"],
        "scribe_v2_realtime"
    );
    assert!(!migrate_local_stt_for_platform_for_test(
        &mut settings,
        true
    ));
}

#[test]
fn external_mlx_selection_migrates_to_a_managed_local_model() {
    let mut settings = json!({
        "model": "whisper-base",
        "cloud_stt_enabled": true,
        "active_cloud_stt_provider": "mlx-audio",
        "cloud_stt_configs": {"mlx-audio": {"model": "mlx-community/parakeet-tdt-0.6b-v3"}}
    });
    assert!(migrate_local_stt_for_platform_for_test(&mut settings, true));
    assert_eq!(settings["model"], "mlx-parakeet-v3");
    assert_eq!(settings["cloud_stt_enabled"], false);
    assert!(settings["cloud_stt_configs"]["mlx-audio"].is_object());
    assert!(!migrate_local_stt_for_platform_for_test(
        &mut settings,
        true
    ));
}

#[test]
fn intel_settings_do_not_migrate_to_an_incompatible_runtime() {
    let mut settings = json!({"model": "whisper-turbo", "cloud_stt_enabled": false});
    let original = settings.clone();
    assert!(!migrate_local_stt_for_platform_for_test(
        &mut settings,
        false
    ));
    assert_eq!(settings, original);
}

#[test]
fn existing_managed_mlx_model_selection_is_preserved() {
    let mut settings =
        json!({"model": "mlx-qwen3-asr-0.6b", "stt_engine": "mlx", "cloud_stt_enabled": false});
    assert!(!migrate_local_stt_for_platform_for_test(
        &mut settings,
        true
    ));
    assert_eq!(settings["model"], "mlx-qwen3-asr-0.6b");
}

#[test]
fn platform_recommendations_never_offer_an_incompatible_engine() {
    for language in ["auto", "fr", "fr-FR", "zh", "ja", "en", "ar"] {
        let mlx = models::recommend_for_platform(language, true);
        assert!(!mlx.is_empty());
        assert_eq!(mlx[0].name, "mlx-whisper-turbo");
        assert!(mlx.iter().all(|model| model.engine_type.as_str() == "mlx"));
        assert!(models::recommend_for_platform(language, false)
            .iter()
            .all(|model| model.engine_type.as_str() != "mlx"));
    }
}

#[test]
fn apple_silicon_does_not_fall_back_to_native_models_or_unselected_mlx_models() {
    if !voiceflow_lib::stt_engine::mlx::runtime::supported() {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let manager =
        voiceflow_lib::stt_engine::UnifiedEngineManager::new(directory.path().join("models"));
    assert_eq!(
        manager.resolve_available_model("whisper-base", "fr"),
        ("mlx".parse().unwrap(), "mlx-whisper-turbo".to_string())
    );
    assert_eq!(
        manager.resolve_available_model("mlx-parakeet-v3", "fr"),
        ("mlx".parse().unwrap(), "mlx-parakeet-v3".to_string())
    );
    assert!(!manager.is_model_downloaded("mlx".parse().unwrap(), "mlx-parakeet-v3"));
    assert!(manager.get_models(EngineType::Whisper).is_empty());
}

#[tokio::test]
#[ignore = "downloads the managed runtime and selected checkpoints; requires VOICEFLOW_MLX_TEST_ROOT and VOICEFLOW_MLX_TEST_WAV"]
async fn managed_mlx_installs_and_transcribes_real_french_audio_without_a_server() {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use voiceflow_lib::stt_engine::{TranscriptionRequest, UnifiedEngineManager};

    let root = std::path::PathBuf::from(
        std::env::var("VOICEFLOW_MLX_TEST_ROOT")
            .expect("set VOICEFLOW_MLX_TEST_ROOT to an isolated directory"),
    );
    let recording = std::env::var("VOICEFLOW_MLX_TEST_WAV").expect("set VOICEFLOW_MLX_TEST_WAV");
    std::fs::create_dir_all(root.join("models")).unwrap();
    let manager = UnifiedEngineManager::new(root.join("models"));
    let decoded = voiceflow_lib::services::transcription_workbench::decode_media_to_mono_16k(
        std::path::Path::new(&recording),
    )
    .unwrap();
    let mut selected = vec!["mlx-whisper-turbo".to_string()];
    if let Ok(second) = std::env::var("VOICEFLOW_MLX_SECOND_MODEL") {
        selected.push(second);
    }
    for model in &selected {
        manager
            .download_model(
                "mlx".parse().unwrap(),
                model,
                Arc::new(AtomicBool::new(false)),
                |downloaded, total| {
                    if downloaded == total {
                        println!("MLX checkpoint files ready: {downloaded}/{total}");
                    }
                },
            )
            .await
            .unwrap();
        assert!(manager.is_model_downloaded("mlx".parse().unwrap(), model));
        manager
            .preload_model("mlx".parse().unwrap(), model)
            .await
            .unwrap();
        assert!(manager.is_model_loaded(model));
        let result = manager
            .transcribe(
                "mlx".parse().unwrap(),
                TranscriptionRequest::new(decoded.samples.clone())
                    .with_model(model)
                    .with_language("fr"),
            )
            .await
            .unwrap();
        println!("Managed {model} transcript: {}", result.text);
        let text = result.text.to_lowercase();
        assert!(text.contains("voix française"), "{}", result.text);
        assert!(text.contains("mac"), "{}", result.text);
        assert_eq!(result.engine.as_str(), "mlx");
        assert_eq!(
            result.model_load_ms,
            Some(0),
            "Explicit-language inference must reuse the preloaded MLX worker"
        );
        let french = TranscriptionRequest::new(decoded.samples.clone())
            .with_model(model)
            .with_language("fr");
        let automatic = TranscriptionRequest::new(decoded.samples.clone()).with_model(model);
        let (french_result, automatic_result) = tokio::join!(
            manager.transcribe(EngineType::Mlx, french),
            manager.transcribe(EngineType::Mlx, automatic),
        );
        for result in [french_result.unwrap(), automatic_result.unwrap()] {
            assert_eq!(result.model_load_ms, Some(0));
            assert!(
                result.text.to_lowercase().contains("mac"),
                "{}",
                result.text
            );
        }
        manager.unload_model();
        assert!(!manager.is_model_loaded(model));
    }
    assert_eq!(
        std::fs::read_dir(
            voiceflow_lib::stt_engine::mlx::runtime::runtime_dir(&root.join("models"))
                .join("audio")
        )
        .unwrap()
        .count(),
        0
    );
}
