use voiceflow_lib::polish_engine::{mlx, PolishEngineType, UnifiedPolishManager};

#[test]
fn managed_polish_display_names_identify_mlx_without_changing_model_ids() {
    for model in mlx::models::MODELS {
        assert!(model.name.ends_with(" · MLX"), "{}", model.name);
        assert!(model.id.starts_with("mlx-"));
        assert_eq!(
            UnifiedPolishManager::get_engine_by_model_id(model.id),
            Some(PolishEngineType::Mlx)
        );
    }
}

#[test]
fn managed_polish_is_a_distinct_local_engine() {
    assert_eq!("mlx".parse::<PolishEngineType>().unwrap().as_str(), "mlx");
    assert_eq!(
        UnifiedPolishManager::get_engine_by_model_id("mlx-qwen3-0.6b"),
        Some(PolishEngineType::Mlx)
    );
    assert_eq!(
        UnifiedPolishManager::get_engine_by_model_id("mlx-qwen3-1.7b"),
        Some(PolishEngineType::Mlx)
    );
    assert_eq!(
        UnifiedPolishManager::get_engine_by_model_id("qwen3.5-0.8b"),
        Some(PolishEngineType::Qwen)
    );
}

#[test]
fn catalogue_only_exposes_curated_complete_mlx_manifests() {
    assert_eq!(mlx::models::MODELS.len(), 2);
    for model in mlx::models::MODELS {
        assert_eq!(model.revision.len(), 40);
        assert!(model.repository.starts_with("mlx-community/"));
        for required in [
            "config.json",
            "model.safetensors",
            "tokenizer.json",
            "tokenizer_config.json",
        ] {
            assert!(model.files.iter().any(|file| file.name == required));
        }
        for file in model.files {
            assert!(file.bytes > 0);
            assert_eq!(file.sha256.len(), 64);
            assert!(!file.name.contains('/'));
        }
    }
    assert!(mlx::models::find("../../arbitrary.gguf").is_none());
}

#[test]
fn runtime_is_separate_from_verified_transcription_runtime() {
    let root = tempfile::tempdir().unwrap();
    let models = root.path().join("models");
    let polish = mlx::runtime::runtime_dir(&models);
    let stt = voiceflow_lib::stt_engine::mlx::runtime::runtime_dir(&models);
    assert_ne!(polish, stt);
    assert!(polish.to_string_lossy().contains("mlx-lm-0.32.2"));
    assert!(!mlx::runtime::installed(&models));
    assert!(!mlx::models::downloaded(&models, "mlx-qwen3-0.6b"));
}

#[test]
fn incompatible_devices_do_not_offer_mlx_models() {
    let native = mlx::models::for_platform(false);
    assert!(native.is_empty());
    assert_eq!(mlx::models::for_platform(true).len(), 2);
}

#[test]
fn mlx_models_do_not_invent_latency_or_template_quality_ratings() {
    for model in mlx::models::MODELS {
        let profile = voiceflow_lib::polish_engine::polish_model_latency_profile(model.id);
        assert_eq!(
            serde_json::to_value(&profile).unwrap()["class"],
            "unmeasured"
        );
        assert_eq!(profile.code, "unmeasured");
        assert!(profile.recommended_templates.is_empty());
        assert!(profile.caution_templates.is_empty());
    }
}

#[test]
fn complete_sizes_without_verified_manifest_never_report_ready() {
    let root = tempfile::tempdir().unwrap();
    let model = &mlx::models::MODELS[0];
    let directory = root.path().join(model.id);
    std::fs::create_dir(&directory).unwrap();
    for file in model.files {
        std::fs::File::create(directory.join(file.name))
            .unwrap()
            .set_len(file.bytes)
            .unwrap();
    }
    assert!(!mlx::models::files_complete(root.path(), model.id));
}

#[test]
fn mlx_compatibility_is_authoritatively_unsupported_on_intel() {
    use voiceflow_lib::polish_engine::{
        assess_polish_model_compatibility, DeviceProfile, PolishModelCompatibilityLevel,
    };
    let device = DeviceProfile {
        platform: "macos",
        arch: "x86_64",
        logical_cpu_count: 16,
        total_memory_mb: Some(64 * 1024),
    };
    let compatibility = assess_polish_model_compatibility("mlx-qwen3-0.6b", &device);
    assert_eq!(
        compatibility.level,
        PolishModelCompatibilityLevel::Unsupported
    );
    assert_eq!(compatibility.code, "platform_unsupported");
}

#[tokio::test]
async fn cancelled_install_does_not_publish_runtime_readiness() {
    use std::sync::{atomic::AtomicBool, Arc};
    let root = tempfile::tempdir().unwrap();
    let models = root.path().join("models");
    assert!(
        mlx::runtime::install(&models, Arc::new(AtomicBool::new(true)))
            .await
            .is_err()
    );
    assert!(!mlx::runtime::installed(&models));
    assert!(!mlx::runtime::runtime_dir(&models)
        .join(".runtime-ready")
        .exists());
}

#[test]
fn polish_lock_pins_all_binary_dependencies_with_hashes() {
    let lock = include_str!("../src/polish_engine/mlx/requirements.lock");
    for dependency in lock
        .split('\n')
        .filter(|line| !line.starts_with([' ', '#']) && !line.trim().is_empty())
    {
        assert!(
            dependency.contains("=="),
            "Unpinned dependency: {dependency}"
        );
        let offset = lock.find(dependency).unwrap() + dependency.len();
        assert!(lock[offset..].trim_start().starts_with("--hash=sha256:"));
    }
    for pinned in ["mlx-lm==0.32.2", "mlx==0.32.4", "mlx-metal==0.32.4"] {
        assert!(lock.contains(pinned));
    }
    assert!(!lock.contains("mlx-audio=="));
}
