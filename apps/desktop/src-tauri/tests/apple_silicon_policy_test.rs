use serde_json::json;
use voiceflow_lib::commands::settings::migrate_local_stt_for_platform_for_test;
use voiceflow_lib::polish_engine::{
    self as polish, DeviceProfile, PolishEngine, PolishEngineType, PolishModelCompatibilityLevel,
    PolishRequest, UnifiedPolishManager,
};
use voiceflow_lib::stt_engine::{models, EngineType, UnifiedEngineManager};

#[test]
fn polish_engine_policy_keeps_cloud_and_native_engines_on_other_hardware() {
    for apple_silicon in [true, false] {
        let engines: Vec<_> = PolishEngineType::all()
            .into_iter()
            .filter(|engine| engine.is_available_for_platform(apple_silicon))
            .collect();
        assert_eq!(
            engines,
            if apple_silicon {
                vec![
                    PolishEngineType::Mlx,
                    PolishEngineType::Apple,
                    PolishEngineType::Cloud,
                ]
            } else {
                vec![
                    PolishEngineType::Qwen,
                    PolishEngineType::Lfm,
                    PolishEngineType::Gemma,
                    PolishEngineType::Glm,
                    PolishEngineType::Apple,
                    PolishEngineType::Cloud,
                ]
            }
        );
    }
}

#[test]
fn apple_silicon_stt_catalogue_keeps_only_mlx_and_apple() {
    let mut engines: Vec<_> = models::for_platform(true)
        .iter()
        .map(|model| model.engine_type.as_str())
        .collect();
    engines.sort_unstable();
    engines.dedup();
    assert_eq!(engines, ["apple", "mlx"]);
    assert!(models::for_platform(true)
        .iter()
        .any(|model| model.name == models::APPLE_SPEECH.name));
}

#[test]
fn apple_speech_migration_preserves_selection_and_vendor_cloud_configuration() {
    let mut settings = json!({
        "model": models::APPLE_SPEECH.name,
        "stt_engine": "apple",
        "cloud_stt_enabled": true,
        "active_cloud_stt_provider": "elevenlabs",
        "cloud_stt_configs": {"elevenlabs": {"api_key": "test-only", "model": "scribe_v2"}},
        "stt_engine_language": "fr"
    });
    let original = settings.clone();
    assert!(!migrate_local_stt_for_platform_for_test(
        &mut settings,
        true
    ));
    assert_eq!(settings, original);
    assert!(!migrate_local_stt_for_platform_for_test(
        &mut settings,
        true
    ));
}

#[test]
fn native_polish_compatibility_is_excluded_only_on_apple_silicon() {
    for (platform, arch, excluded) in [
        ("macos", "aarch64", true),
        ("macos", "x86_64", false),
        ("windows", "x86_64", false),
        ("linux", "aarch64", false),
    ] {
        let device = DeviceProfile {
            platform,
            arch,
            logical_cpu_count: 16,
            total_memory_mb: Some(64 * 1024),
        };
        for model in [
            "qwen3.5-0.8b",
            "lfm2.5-1.2b",
            "gemma-2b-it",
            "glm-4.7-flash-reap-23b-a3b",
        ] {
            let result = polish::assess_polish_model_compatibility(model, &device);
            if excluded {
                assert_eq!(
                    result.level,
                    PolishModelCompatibilityLevel::Unsupported,
                    "{model}"
                );
                assert_eq!(result.code, "platform_unsupported");
            } else {
                assert_eq!(
                    result.level,
                    PolishModelCompatibilityLevel::Smooth,
                    "{platform}/{arch}/{model}"
                );
            }
        }
    }
}

#[test]
fn host_polish_catalogue_and_engines_enforce_hardware_policy() {
    let manager = UnifiedPolishManager::new();
    let models = polish::get_all_polish_models();
    if polish::mlx::runtime::supported() {
        assert_eq!(models.len(), 2);
        assert!(models
            .iter()
            .all(|model| model.engine_type == PolishEngineType::Mlx));
        let mut engines = manager.available_engines();
        engines.sort_by_key(PolishEngineType::as_str);
        assert_eq!(engines, [PolishEngineType::Apple, PolishEngineType::Mlx]);
    } else {
        for engine in [
            PolishEngineType::Qwen,
            PolishEngineType::Lfm,
            PolishEngineType::Gemma,
            PolishEngineType::Glm,
        ] {
            assert!(models.iter().any(|model| model.engine_type == engine));
            assert!(manager.available_engines().contains(&engine));
        }
        assert!(!models
            .iter()
            .any(|model| model.engine_type == PolishEngineType::Mlx));
    }
    assert_eq!(
        UnifiedPolishManager::get_engine_by_model_id("qwen3.5-0.8b"),
        Some(PolishEngineType::Qwen)
    );
    assert_eq!(
        manager
            .get_model_filename(PolishEngineType::Qwen, "qwen3.5-0.8b")
            .as_deref(),
        Some("Qwen3.5-0.8B-Q5_K_M.gguf")
    );
}

#[tokio::test]
async fn excluded_polish_engines_cannot_load_preload_or_infer() {
    let manager = UnifiedPolishManager::new();
    for (engine, model) in [
        (PolishEngineType::Qwen, "qwen3.5-0.8b"),
        (PolishEngineType::Lfm, "lfm2.5-1.2b"),
        (PolishEngineType::Gemma, "gemma-2b-it"),
        (PolishEngineType::Glm, "glm-4.7-flash-reap-23b-a3b"),
    ] {
        if polish::mlx::runtime::supported() {
            assert!(!manager.is_model_downloaded(engine, model));
            assert!(!manager.is_model_loaded(engine, model));
            assert!(!manager.is_runtime_ready_for(engine));
            assert!(manager
                .load_model(engine, model)
                .unwrap_err()
                .contains("not available on this computer"));
            assert!(manager
                .preload_model(engine, model)
                .await
                .unwrap_err()
                .contains("not available on this computer"));
            let request = PolishRequest::new("Bonjour", "Correct", "fr");
            assert!(manager
                .polish(engine, request)
                .await
                .unwrap_err()
                .contains("not available on this computer"));
        } else {
            assert_eq!(
                manager.load_model(engine, "unknown-model").unwrap_err(),
                "Model not found: unknown-model"
            );
        }
    }
}

#[tokio::test]
async fn direct_native_polish_engine_cannot_bypass_platform_policy() {
    let engines: Vec<Box<dyn PolishEngine>> = vec![
        Box::new(polish::qwen::QwenPolishEngine::new()),
        Box::new(polish::lfm::LfmPolishEngine::new()),
        Box::new(polish::gemma::GemmaPolishEngine::new()),
        Box::new(polish::glm::GlmPolishEngine::new()),
    ];
    for engine in engines {
        let error = engine
            .polish(
                PolishRequest::new("Bonjour", "Correct", "fr")
                    .with_model("missing-policy-fixture.gguf"),
            )
            .await
            .unwrap_err();
        if polish::mlx::runtime::supported() {
            assert!(error.contains("not available on this computer"), "{error}");
        } else {
            assert!(error.contains("not found"), "{error}");
        }
    }
}

#[test]
fn explicit_apple_stt_selection_is_not_replaced_with_mlx() {
    let directory = tempfile::tempdir().unwrap();
    let manager = UnifiedEngineManager::new(directory.path().join("models"));
    if polish::mlx::runtime::supported() {
        assert_eq!(
            manager.resolve_available_model(models::APPLE_SPEECH.name, "fr"),
            (EngineType::Apple, models::APPLE_SPEECH.name.to_string())
        );
        assert_eq!(
            UnifiedEngineManager::available_engines(),
            [EngineType::Mlx, EngineType::Apple, EngineType::Cloud]
        );
    } else {
        assert!(UnifiedEngineManager::available_engines().contains(&EngineType::Whisper));
    }
}

#[tokio::test]
async fn native_stt_operations_preserve_legacy_files_and_allow_explicit_deletion() {
    use std::sync::{atomic::AtomicBool, Arc};
    use voiceflow_lib::stt_engine::TranscriptionRequest;

    let directory = tempfile::tempdir().unwrap();
    let manager = UnifiedEngineManager::new(directory.path().join("models"));
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        for model in models::for_platform(false)
            .into_iter()
            .filter(|model| !model.built_in)
        {
            let model_dir = directory.path().join("models").join(model.name);
            std::fs::create_dir_all(&model_dir).unwrap();
            let file = model_dir.join(model.files[0].filename);
            std::fs::write(&file, b"preserved legacy checkpoint").unwrap();
            let download = manager
                .download_model(
                    model.engine_type,
                    model.name,
                    Arc::new(AtomicBool::new(false)),
                    |_, _| {},
                )
                .await;
            assert!(download
                .unwrap_err()
                .contains("not available on this computer"));
            assert!(manager
                .load_model(model.engine_type, model.name)
                .unwrap_err()
                .contains("not available on this computer"));
            assert!(manager
                .preload_model(model.engine_type, model.name)
                .await
                .unwrap_err()
                .contains("not available on this computer"));
            assert!(manager
                .transcribe(
                    model.engine_type,
                    TranscriptionRequest::new(vec![]).with_model(model.name)
                )
                .await
                .unwrap_err()
                .contains("not available on this computer"));
            assert!(!manager.is_model_downloaded(model.engine_type, model.name));
            assert!(!manager.is_model_loaded(model.name));
            assert_eq!(
                std::fs::read(&file).unwrap(),
                b"preserved legacy checkpoint"
            );
            assert_eq!(
                UnifiedEngineManager::get_engine_by_model_name(model.name),
                Some(model.engine_type)
            );
            manager.delete_model(model.engine_type, model.name).unwrap();
            assert!(!model_dir.exists());
        }
        assert!(!directory.path().join("runtimes").exists());
    } else {
        let error = manager
            .download_model(
                EngineType::Whisper,
                "unknown-model",
                Arc::new(AtomicBool::new(false)),
                |_, _| {},
            )
            .await
            .unwrap_err();
        assert_eq!(error, "Unknown model: unknown-model");
    }
}
