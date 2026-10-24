use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tracing::{error, info, instrument};

use super::apple::{self, AppleSpeechEngine, AppleSpeechStatus};
use super::mlx::{self, MlxEngine};
use super::models;
use super::sherpa_onnx::SherpaOnnxEngine;
use super::traits::{EngineType, TranscriptionRequest, TranscriptionResult};
use crate::utils::AppPaths;
use crate::utils::{
    download, downloaded_file_is_complete, remove_download_artifacts, DownloadOptions,
    HuggingFaceSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceProvider {
    Cpu,
    CoreML,
    Cuda,
}

impl InferenceProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            InferenceProvider::Cpu => "cpu",
            InferenceProvider::CoreML => "coreml",
            InferenceProvider::Cuda => "cuda",
        }
    }

    pub fn from_gpu_setting(gpu_acceleration: bool) -> Self {
        if gpu_acceleration {
            #[cfg(target_os = "macos")]
            {
                InferenceProvider::CoreML
            }
            #[cfg(not(target_os = "macos"))]
            {
                InferenceProvider::Cuda
            }
        } else {
            InferenceProvider::Cpu
        }
    }
}

impl fmt::Display for InferenceProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

type EngineCacheKey = (EngineType, String, Option<String>, InferenceProvider);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub display_name: String,
    pub size_mb: u64,
    pub filename: String,
    pub downloaded: bool,
    #[serde(default)]
    pub downloadable: bool,
    pub speed_score: u8,
    pub accuracy_score: u8,
    pub engine: String,
    /// Catalogue language restrictions. Empty means no restriction is declared.
    #[serde(default)]
    pub language_codes: Vec<String>,
    /// Ships with the operating system: no download size, cannot be deleted.
    pub built_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_ready: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedModel {
    pub engine_type: EngineType,
    pub model_name: String,
    pub display_name: String,
    pub size_mb: u64,
    pub speed_score: u8,
    pub accuracy_score: u8,
    pub downloaded: bool,
}

const QWEN3_ASR_MODEL_NAME: &str = "qwen3-asr-0.6b-int8";
const QWEN3_ASR_ARCHIVE_TOP_DIR: &str = "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25";
const QWEN3_ASR_ARCHIVE_FILE: &str = "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2";
const QWEN3_ASR_RELEASE_URL: &str = concat!(
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/",
    "sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25.tar.bz2"
);
const QWEN3_ASR_ARCHIVE_SIZE_BYTES: u64 = 878_702_423;
const SILERO_VAD_REPO: &str = "onnx-community/silero-vad";
const SILERO_VAD_REMOTE_FILE: &str = "onnx/model.onnx";
const SILERO_VAD_LOCAL_FILE: &str = "silero_vad.onnx";

pub struct UnifiedEngineManager {
    models_dir: PathBuf,
    engine_cache: Arc<Mutex<Option<(EngineCacheKey, EngineInstance)>>>,
    provider: Arc<Mutex<InferenceProvider>>,
    /// Dictation language setting; the Apple engine needs it to check assets.
    dictation_language: Arc<Mutex<String>>,
    /// Last Apple engine status and the language it was computed for. Bridge
    /// queries block, and model listings run on the main thread where a
    /// blocking query freezes the app, so they only run on background threads.
    apple_status_cache: Arc<Mutex<Option<(String, AppleSpeechStatus)>>>,
    apple_status_refreshing: Arc<AtomicBool>,
    apple_status_changed: Arc<tokio::sync::Notify>,
}

impl UnifiedEngineManager {
    pub fn new(models_dir: PathBuf) -> Self {
        info!(models_dir = ?models_dir, "engine_manager_initialized");
        Self {
            models_dir,
            engine_cache: Arc::new(Mutex::new(None)),
            provider: Arc::new(Mutex::new(InferenceProvider::Cpu)),
            dictation_language: Arc::new(Mutex::new("auto".to_string())),
            apple_status_cache: Arc::new(Mutex::new(None)),
            apple_status_refreshing: Arc::new(AtomicBool::new(false)),
            apple_status_changed: Arc::new(tokio::sync::Notify::new()),
        }
    }

    pub fn set_dictation_language(&self, language: &str) {
        *self.dictation_language.lock().unwrap() = language.to_string();
        self.refresh_apple_status();
    }

    pub fn dictation_language(&self) -> String {
        self.dictation_language.lock().unwrap().clone()
    }

    /// Apple engine availability for the current dictation language, read
    /// from the cache. Until a background query answers, the engine is treated
    /// as unsupported, which hides it instead of blocking the caller.
    pub(crate) fn apple_status(&self) -> AppleSpeechStatus {
        let language = self.dictation_language();
        if let Some((cached_language, status)) = self.apple_status_cache.lock().unwrap().as_ref() {
            if *cached_language == language {
                return *status;
            }
        }
        self.refresh_apple_status();
        AppleSpeechStatus::OsUnsupported
    }

    /// Queries the Apple engine status on a background thread.
    pub fn refresh_apple_status(&self) {
        if self
            .apple_status_refreshing
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return;
        }
        let language = self.dictation_language();
        let cache = Arc::clone(&self.apple_status_cache);
        let refreshing = Arc::clone(&self.apple_status_refreshing);
        let changed = Arc::clone(&self.apple_status_changed);
        std::thread::spawn(move || {
            let status = apple::bridge::status(&language);
            info!(language = %language, status = ?status, "apple_speech_status_refreshed");
            *cache.lock().unwrap() = Some((language, status));
            refreshing.store(false, std::sync::atomic::Ordering::Release);
            changed.notify_waiters();
        });
    }

    async fn resolve_apple_status(
        &self,
        deadline: std::time::Duration,
    ) -> Result<AppleSpeechStatus, String> {
        tokio::time::timeout(deadline, async {
            loop {
                let changed = self.apple_status_changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                let language = self.dictation_language();
                if let Some((cached_language, status)) =
                    self.apple_status_cache.lock().unwrap().as_ref()
                {
                    if *cached_language == language {
                        return *status;
                    }
                }
                self.refresh_apple_status();
                changed.await;
            }
        })
        .await
        .map_err(|_| "Apple Speech availability check timed out; retry model loading".to_string())
    }

    pub fn set_provider(&self, gpu_acceleration: bool) {
        let new_provider = InferenceProvider::from_gpu_setting(gpu_acceleration);
        let mut provider = self.provider.lock().unwrap();
        if *provider != new_provider {
            info!(old = %*provider, new = %new_provider, "provider_updated");
            *provider = new_provider;
        }
    }

    pub fn default_models_dir() -> PathBuf {
        AppPaths::models_dir()
    }

    fn validate_model_policy(engine_type: EngineType, model_name: &str) -> Result<(), String> {
        if engine_type == EngineType::Cloud {
            return Ok(());
        }
        let definition = models::find_by_name(model_name);
        if (mlx::runtime::supported() || definition.is_some())
            && !definition.is_some_and(models::is_available)
        {
            return Err(format!(
                "Model '{model_name}' is not available on this computer"
            ));
        }
        if let Some(model) = definition {
            if model.engine_type != engine_type {
                return Err(format!(
                    "Model '{}' is for {:?}, not {:?}",
                    model_name, model.engine_type, engine_type
                ));
            }
        }
        Ok(())
    }

    fn create_engine_instance(
        &self,
        engine_type: EngineType,
        version: &str,
        language: Option<&str>,
    ) -> Result<EngineInstance, String> {
        Self::validate_model_policy(engine_type, version)?;
        match engine_type {
            EngineType::Whisper | EngineType::SenseVoice | EngineType::Qwen3Asr => {
                let model_def = models::find_by_name(version)
                    .ok_or_else(|| format!("Unknown model: {}", version))?;
                if model_def.engine_type != engine_type {
                    return Err(format!(
                        "Model '{}' is for {:?}, not {:?}",
                        version, model_def.engine_type, engine_type
                    ));
                }
                let provider = *self.provider.lock().unwrap();
                let engine =
                    SherpaOnnxEngine::new(&self.models_dir, model_def, language, provider)?;
                Ok(EngineInstance::Local(engine))
            }
            EngineType::Apple => Ok(EngineInstance::Apple(AppleSpeechEngine::new(language))),
            EngineType::Mlx => {
                if !self.is_model_downloaded(engine_type, version) {
                    return Err(format!("MLX model '{version}' is not ready. Download it in Voice Flow to install the model and its runtime."));
                }
                Ok(EngineInstance::Mlx(MlxEngine::new(
                    &self.models_dir,
                    version,
                )?))
            }
            EngineType::Cloud => {
                Err("Cloud STT uses streaming lifecycle, not batch transcription.".to_string())
            }
        }
    }

    pub(crate) fn get_or_create_engine(
        &self,
        engine_type: EngineType,
        version: &str,
        language: Option<&str>,
    ) -> Result<EngineInstance, String> {
        Self::validate_model_policy(engine_type, version)?;
        let provider = *self.provider.lock().unwrap();
        let cache_key = (
            engine_type,
            version.to_string(),
            if engine_type == EngineType::Mlx {
                None
            } else {
                language.map(str::to_string)
            },
            provider,
        );
        let mut cache = self.engine_cache.lock().unwrap();

        if let Some((cached_key, cached_engine)) = cache.as_ref() {
            if *cached_key == cache_key {
                return Ok(cached_engine.clone());
            }
        }

        let engine = self.create_engine_instance(engine_type, version, language)?;
        if let Some((_, EngineInstance::Mlx(previous))) = cache.as_ref() {
            previous.shutdown();
        }
        *cache = Some((cache_key, engine.clone()));
        Ok(engine)
    }

    pub(crate) fn clear_cache(&self) {
        let mut cache = self.engine_cache.lock().unwrap();
        if let Some((_, EngineInstance::Mlx(engine))) = cache.as_ref() {
            engine.shutdown();
        }
        *cache = None;
    }

    pub(crate) fn clear_language_dependent_cache(&self) {
        let mut cache = self.engine_cache.lock().unwrap();
        if !matches!(cache.as_ref(), Some((_, EngineInstance::Mlx(_)))) {
            *cache = None;
        }
    }

    pub fn unload_model(&self) {
        self.clear_cache();
    }

    #[instrument(
        skip(self, request),
        fields(
            engine = engine_type.as_str(),
            model = request.model_name.as_deref().unwrap_or("default"),
            language = request.language.as_deref().unwrap_or("auto"),
            samples = request.samples.len(),
        ),
        ret,
        err
    )]
    pub async fn transcribe(
        &self,
        engine_type: EngineType,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResult, String> {
        let model = request
            .model_name
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let lang = request
            .language
            .clone()
            .unwrap_or_else(|| "auto".to_string());
        info!("transcription_started");

        let version =
            if request.model_name.as_deref() == Some("default") || request.model_name.is_none() {
                let default_model = if engine_type == EngineType::Apple {
                    &models::APPLE_SPEECH
                } else {
                    models::default_for_language(&lang)
                };
                default_model.name.to_string()
            } else {
                request
                    .model_name
                    .clone()
                    .unwrap_or_else(|| "default".to_string())
            };

        let lang_for_engine = if lang == "auto" {
            None
        } else {
            Some(lang.as_str())
        };
        let engine = self.get_or_create_engine(engine_type, &version, lang_for_engine)?;

        let result = engine.transcribe(request).await;

        match &result {
            Ok(r) => {
                info!(engine = engine_type.as_str(), model = %model, text_len = r.text.len(), duration_ms = r.total_ms, "transcription_completed")
            }
            Err(e) => {
                error!(engine = engine_type.as_str(), model = %model, error = %e, "transcription_failed")
            }
        }

        result
    }

    pub fn available_engines() -> Vec<EngineType> {
        if mlx::runtime::supported() {
            vec![EngineType::Mlx, EngineType::Apple, EngineType::Cloud]
        } else {
            EngineType::all()
                .into_iter()
                .filter(|engine| *engine != EngineType::Mlx)
                .collect()
        }
    }

    // ==================== Model Management Functions ====================

    pub fn get_models(&self, engine_type: EngineType) -> Vec<ModelInfo> {
        self.listed_models()
            .filter(|def| def.engine_type == engine_type)
            .map(|def| self.model_info(def))
            .collect()
    }

    pub fn get_all_models(&self) -> Vec<ModelInfo> {
        self.listed_models()
            .map(|def| self.model_info(def))
            .collect()
    }

    pub async fn get_models_with_status(
        &self,
        engine_type: Option<EngineType>,
    ) -> Result<Vec<ModelInfo>, String> {
        if engine_type.is_none() || engine_type == Some(EngineType::Apple) {
            self.resolve_apple_status(std::time::Duration::from_secs(10))
                .await?;
        }
        Ok(match engine_type {
            Some(engine) => self.get_models(engine),
            None => self.get_all_models(),
        })
    }

    /// Models this system can run. The Apple engine is hidden before macOS 26.
    fn listed_models(&self) -> impl Iterator<Item = &'static models::ModelDefinition> + '_ {
        let apple_supported = self.apple_status() != AppleSpeechStatus::OsUnsupported;
        models::available()
            .into_iter()
            .filter(move |def| def.engine_type != EngineType::Apple || apple_supported)
    }

    fn model_info(&self, def: &models::ModelDefinition) -> ModelInfo {
        let apple_status = (def.engine_type == EngineType::Apple).then(|| self.apple_status());
        ModelInfo {
            name: def.name.to_string(),
            display_name: def.display_name.to_string(),
            size_mb: def.size_mb as u64,
            filename: def.name.to_string(),
            downloaded: apple_status.map_or_else(
                || self.is_model_downloaded(def.engine_type, def.name),
                |status| status == AppleSpeechStatus::Ready,
            ),
            downloadable: !def.built_in || apple_status == Some(AppleSpeechStatus::AssetsMissing),
            speed_score: def.speed_score,
            accuracy_score: def.accuracy_score,
            engine: def.engine_type.as_str().to_string(),
            language_codes: def
                .prefer_lang
                .iter()
                .map(|code| (*code).to_string())
                .collect(),
            built_in: def.built_in,
            runtime_ready: (def.engine_type == EngineType::Mlx)
                .then(|| mlx::runtime::installed(&self.models_dir)),
        }
    }

    pub fn get_engine_by_model_name(model_name: &str) -> Option<EngineType> {
        if model_name == "cloud" {
            Some(EngineType::Cloud)
        } else {
            models::find_by_name(model_name).map(|m| m.engine_type)
        }
    }

    pub fn is_model_downloaded(&self, engine_type: EngineType, model_name: &str) -> bool {
        let model_def = match models::find_by_name(model_name) {
            Some(def) => def,
            None => return false,
        };

        if model_def.engine_type != engine_type || !models::is_available(model_def) {
            return false;
        }

        if engine_type == EngineType::Apple {
            return self.apple_status() == AppleSpeechStatus::Ready;
        }

        if engine_type == EngineType::Mlx && !mlx::runtime::installed(&self.models_dir) {
            return false;
        }

        let model_subdir = self.models_dir.join(model_def.name);
        model_def.files.iter().all(|file| {
            if engine_type == EngineType::Mlx {
                return std::fs::metadata(model_subdir.join(file.filename)).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.len() == file.estimated_size_bytes()
                });
            }
            downloaded_file_is_complete(
                &model_subdir.join(file.filename),
                Some(file.minimum_complete_bytes()),
            )
        })
    }

    pub fn resolve_available_model(
        &self,
        requested_model: &str,
        language: &str,
    ) -> (EngineType, String) {
        if mlx::runtime::supported() {
            let model = models::find_by_name(requested_model)
                .filter(|model| models::is_available(model))
                .unwrap_or_else(|| models::default_for_language(language));
            return (model.engine_type, model.name.to_string());
        }
        if let Some(model_def) = models::find_by_name(requested_model) {
            if self.is_model_downloaded(model_def.engine_type, model_def.name) {
                return (model_def.engine_type, model_def.name.to_string());
            }
        }

        let candidates = models::recommend_by_language(language);
        for model_def in &candidates {
            if self.is_model_downloaded(model_def.engine_type, model_def.name) {
                tracing::warn!(
                    requested = requested_model,
                    fallback = model_def.name,
                    "configured_model_not_downloaded_falling_back"
                );
                return (model_def.engine_type, model_def.name.to_string());
            }
        }

        // Built-in engines are opt-in: only an explicit selection uses them.
        for model_def in models::available().into_iter().filter(|def| !def.built_in) {
            if self.is_model_downloaded(model_def.engine_type, model_def.name) {
                tracing::warn!(
                    requested = requested_model,
                    fallback = model_def.name,
                    "no_recommended_model_available_using_first_downloaded"
                );
                return (model_def.engine_type, model_def.name.to_string());
            }
        }

        let default = models::default_for_language(language);
        (default.engine_type, default.name.to_string())
    }

    /// First downloaded model for `language` whose engine is not `excluded`,
    /// used when the selected engine fails at runtime.
    pub fn fallback_model(&self, language: &str, excluded: EngineType) -> Option<String> {
        if mlx::runtime::supported() {
            return None;
        }
        models::recommend_by_language(language)
            .into_iter()
            .chain(models::available())
            .filter(|def| def.engine_type != excluded && !def.built_in)
            .find(|def| self.is_model_downloaded(def.engine_type, def.name))
            .map(|def| def.name.to_string())
    }

    pub fn get_model_path(&self, engine_type: EngineType, model_name: &str) -> PathBuf {
        if matches!(engine_type, EngineType::Cloud | EngineType::Apple) {
            return PathBuf::new();
        }
        self.models_dir.join(model_name)
    }

    pub async fn download_model<F>(
        &self,
        engine_type: EngineType,
        model_name: &str,
        cancel_flag: Arc<AtomicBool>,
        progress_callback: F,
    ) -> Result<PathBuf, String>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        let model_def = models::find_by_name(model_name)
            .ok_or_else(|| format!("Unknown model: {}", model_name))?;

        if model_def.engine_type != engine_type {
            return Err(format!(
                "Model '{}' is for {:?}, not {:?}",
                model_name, model_def.engine_type, engine_type
            ));
        }

        if !models::is_available(model_def) {
            return Err(format!(
                "Model '{model_name}' is not available on this computer"
            ));
        }
        if engine_type == EngineType::Mlx {
            mlx::runtime::install(&self.models_dir, cancel_flag.clone()).await?;
            self.reuse_cached_mlx_model(model_def, &cancel_flag).await?;
        }

        if engine_type == EngineType::Qwen3Asr {
            return self
                .download_qwen3_asr_model(model_name, cancel_flag, progress_callback)
                .await;
        }

        if engine_type == EngineType::Apple {
            let language = self.dictation_language();
            info!(language = %language, "apple_speech_assets_install_started");
            let status = tokio::task::spawn_blocking(move || {
                apple::bridge::install_assets(&language)?;
                Ok::<_, String>((language.clone(), apple::bridge::status(&language)))
            })
            .await
            .map_err(|e| format!("Apple speech asset task failed: {e}"))??;
            *self.apple_status_cache.lock().unwrap() = Some(status);
            progress_callback(1, 1);
            info!(model = %model_name, "apple_speech_assets_installed");
            return Ok(PathBuf::new());
        }

        let repo = model_def
            .repository
            .ok_or_else(|| format!("Model '{}' has no download repository", model_name))?;

        let model_subdir = self.models_dir.join(model_name);
        std::fs::create_dir_all(&model_subdir)
            .map_err(|e| format!("Failed to create model directory: {}", e))?;

        let total_size_bytes: u64 = model_def
            .files
            .iter()
            .map(|file| file.estimated_size_bytes())
            .sum();

        let mut last_output_path: Option<PathBuf> = None;
        let progress_cb = Arc::new(progress_callback) as Arc<dyn Fn(u64, u64) + Send + Sync>;
        let completed_bytes = Arc::new(std::sync::atomic::AtomicU64::new(0u64));

        for model_file in model_def.files {
            let output_path = model_subdir.join(model_file.filename);

            let complete = if engine_type == EngineType::Mlx {
                output_path.metadata().is_ok_and(|metadata| {
                    metadata.is_file() && metadata.len() == model_file.estimated_size_bytes()
                })
            } else {
                downloaded_file_is_complete(&output_path, Some(model_file.minimum_complete_bytes()))
            };
            if complete {
                let file_bytes = output_path
                    .metadata()
                    .map(|metadata| metadata.len())
                    .unwrap_or_else(|_| model_file.estimated_size_bytes());
                completed_bytes.fetch_add(file_bytes, std::sync::atomic::Ordering::Relaxed);
                progress_cb(
                    completed_bytes.load(std::sync::atomic::Ordering::Relaxed),
                    total_size_bytes,
                );
                info!(file = %model_file.filename, "file_already_exists_skipping");
                last_output_path = Some(output_path);
                continue;
            }

            if output_path.exists() {
                info!(file = %model_file.filename, "incomplete_model_file_redownloading");
                remove_download_artifacts(&output_path);
            }

            info!(
                repo = repo,
                file = %model_file.filename,
                "downloading_model_file"
            );

            let source = HuggingFaceSource::new(repo, model_file.source_filename);
            let source = if let Some(revision) = mlx::models::revision(model_name) {
                source.with_revision(revision)
            } else {
                source
            }
            .into_source();
            let urls = source.urls();

            let cb = progress_cb.clone();
            let completed_ref = completed_bytes.clone();
            let file_size_bytes = model_file.estimated_size_bytes();
            let options = DownloadOptions::new(&urls[0], &output_path)
                .with_fallbacks(urls[1..].to_vec())
                .with_cancel_flag(cancel_flag.clone())
                .with_progress_callback(Arc::new(move |downloaded, total| {
                    let file_total = if total > 0 { total } else { file_size_bytes };
                    let file_done = if file_total > 0 { downloaded } else { 0 };
                    let base = completed_ref.load(std::sync::atomic::Ordering::Relaxed);
                    cb(base + file_done, total_size_bytes);
                }))
                .with_minimum_bytes(if engine_type == EngineType::Mlx {
                    model_file.estimated_size_bytes()
                } else {
                    model_file.minimum_complete_bytes()
                })
                .with_model_name(model_name);

            let result = download(options).await?;
            completed_bytes.fetch_add(result.bytes, std::sync::atomic::Ordering::Relaxed);
            last_output_path = Some(result.path);
        }

        if !self.is_model_downloaded(engine_type, model_name) {
            return Err(format!(
                "Model '{}' downloaded but failed completeness validation",
                model_name
            ));
        }

        let output_path = last_output_path.unwrap_or(model_subdir);
        info!(engine = ?engine_type, model = %model_name, path = ?output_path, "model_download_completed");

        Ok(output_path)
    }

    async fn reuse_cached_mlx_model(
        &self,
        model: &models::ModelDefinition,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        let cache = std::env::var_os("HF_HUB_CACHE")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HF_HOME").map(|path| PathBuf::from(path).join("hub")))
            .or_else(|| dirs::home_dir().map(|path| path.join(".cache/huggingface/hub")));
        let Some(cache) = cache else {
            return Ok(());
        };
        let repository = model.repository.ok_or("MLX model has no repository")?;
        let revision =
            mlx::models::revision(model.name).ok_or("MLX model has no pinned revision")?;
        let source_dir = cache
            .join(format!("models--{}", repository.replace('/', "--")))
            .join("snapshots")
            .join(revision);
        let destination_dir = self.models_dir.join(model.name);
        tokio::fs::create_dir_all(&destination_dir)
            .await
            .map_err(|error| error.to_string())?;
        for file in model.files {
            if cancel.load(std::sync::atomic::Ordering::Acquire) {
                return Err("cancelled".to_string());
            }
            let source = source_dir.join(file.source_filename);
            let destination = destination_dir.join(file.filename);
            if destination
                .metadata()
                .is_ok_and(|metadata| metadata.len() == file.estimated_size_bytes())
            {
                continue;
            }
            if source.metadata().is_ok_and(|metadata| {
                metadata.is_file() && metadata.len() == file.estimated_size_bytes()
            }) {
                let temporary = destination.with_extension("cache.tmp");
                tokio::fs::copy(&source, &temporary)
                    .await
                    .map_err(|error| format!("Cannot reuse cached MLX checkpoint: {error}"))?;
                tokio::fs::rename(&temporary, &destination)
                    .await
                    .map_err(|error| format!("Cannot install cached MLX checkpoint: {error}"))?;
                info!(
                    model = model.name,
                    file = file.filename,
                    "mlx_checkpoint_reused_from_cache"
                );
            }
        }
        Ok(())
    }

    async fn download_qwen3_asr_model<F>(
        &self,
        model_name: &str,
        cancel_flag: Arc<AtomicBool>,
        progress_callback: F,
    ) -> Result<PathBuf, String>
    where
        F: Fn(u64, u64) + Send + Sync + 'static,
    {
        if model_name != QWEN3_ASR_MODEL_NAME {
            return Err(format!("Unknown Qwen3-ASR model: {}", model_name));
        }

        let model_subdir = self.models_dir.join(model_name);
        let total_size_bytes = QWEN3_ASR_ARCHIVE_SIZE_BYTES;
        let progress_cb = Arc::new(progress_callback) as Arc<dyn Fn(u64, u64) + Send + Sync>;

        if self.is_model_downloaded(EngineType::Qwen3Asr, model_name) {
            progress_cb(total_size_bytes, total_size_bytes);
            info!(model = %model_name, "qwen3_asr_model_already_downloaded");
            return Ok(model_subdir);
        }

        std::fs::create_dir_all(&self.models_dir)
            .map_err(|e| format!("Failed to create models directory: {}", e))?;

        let archive_path = self.models_dir.join(QWEN3_ASR_ARCHIVE_FILE);
        let staging_dir = self.models_dir.join(format!("{}.extracting", model_name));

        if model_subdir.exists() {
            info!(model = %model_name, path = ?model_subdir, "incomplete_qwen3_asr_model_redownloading");
            std::fs::remove_dir_all(&model_subdir)
                .map_err(|e| format!("Failed to remove incomplete Qwen3-ASR model: {}", e))?;
        }
        if staging_dir.exists() {
            std::fs::remove_dir_all(&staging_dir)
                .map_err(|e| format!("Failed to remove stale Qwen3-ASR staging dir: {}", e))?;
        }
        std::fs::create_dir_all(&staging_dir)
            .map_err(|e| format!("Failed to create Qwen3-ASR staging dir: {}", e))?;

        let cb = progress_cb.clone();
        let options = DownloadOptions::new(QWEN3_ASR_RELEASE_URL, &archive_path)
            .with_cancel_flag(cancel_flag.clone())
            .with_progress_callback(Arc::new(move |downloaded, total| {
                cb(downloaded, if total > 0 { total } else { total_size_bytes });
            }))
            .with_minimum_bytes(QWEN3_ASR_ARCHIVE_SIZE_BYTES)
            .with_model_name(model_name);

        let result = download(options).await?;
        info!(
            model = %model_name,
            archive = ?result.path,
            bytes = result.bytes,
            "qwen3_asr_archive_downloaded"
        );

        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            remove_download_artifacts(&archive_path);
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err("cancelled".to_string());
        }

        let archive_for_extract = archive_path.clone();
        let staging_for_extract = staging_dir.clone();
        tokio::task::spawn_blocking(move || {
            extract_tar_bz2_archive(&archive_for_extract, &staging_for_extract)
        })
        .await
        .map_err(|e| format!("Qwen3-ASR extraction task failed: {}", e))??;

        if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
            let _ = std::fs::remove_dir_all(&staging_dir);
            remove_download_artifacts(&archive_path);
            return Err("cancelled".to_string());
        }

        let extracted_root = staging_dir.join(QWEN3_ASR_ARCHIVE_TOP_DIR);
        if !extracted_root.exists() {
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(format!(
                "Qwen3-ASR archive did not contain expected directory: {}",
                QWEN3_ASR_ARCHIVE_TOP_DIR
            ));
        }

        std::fs::rename(&extracted_root, &model_subdir).map_err(|e| {
            let _ = std::fs::remove_dir_all(&staging_dir);
            format!("Failed to install Qwen3-ASR model: {}", e)
        })?;
        let _ = std::fs::remove_dir_all(model_subdir.join("test_wavs"));
        let _ = std::fs::remove_dir_all(&staging_dir);

        remove_download_artifacts(&archive_path);

        if !self.is_model_downloaded(EngineType::Qwen3Asr, model_name) {
            return Err(
                "Qwen3-ASR archive extracted but required files are incomplete".to_string(),
            );
        }

        progress_cb(total_size_bytes, total_size_bytes);
        info!(model = %model_name, path = ?model_subdir, "qwen3_asr_model_installed");
        Ok(model_subdir)
    }

    #[instrument(skip(self), fields(engine = ?engine_type, model = %model_name), ret, err)]
    pub fn load_model(&self, engine_type: EngineType, model_name: &str) -> Result<(), String> {
        Self::validate_model_policy(engine_type, model_name)?;
        if !self.is_model_downloaded(engine_type, model_name) {
            return Err(format!(
                "Model '{}' not downloaded. Please download it first.",
                model_name
            ));
        }

        info!(
            engine = ?engine_type,
            model = %model_name,
            "model_preload_started"
        );

        let _ = self.get_or_create_engine(engine_type, model_name, None)?;

        info!(
            engine = ?engine_type,
            model = %model_name,
            "model_preloaded"
        );

        Ok(())
    }

    pub async fn preload_model(
        &self,
        engine_type: EngineType,
        model_name: &str,
    ) -> Result<(), String> {
        if engine_type == EngineType::Mlx {
            match self.get_or_create_engine(engine_type, model_name, None)? {
                EngineInstance::Mlx(engine) => engine.preload().await,
                _ => Err("The selected model is not an MLX model".to_string()),
            }
        } else {
            self.load_model(engine_type, model_name)
        }
    }

    pub fn is_model_loaded(&self, model_name: &str) -> bool {
        if !models::find_by_name(model_name).is_some_and(models::is_available) {
            return false;
        }
        self.engine_cache
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(key, engine)| {
                key.1 == model_name
                    && match engine {
                        EngineInstance::Mlx(engine) => engine.is_loaded(),
                        _ => true,
                    }
            })
    }

    pub fn delete_model(&self, engine_type: EngineType, model_name: &str) -> Result<(), String> {
        let definition = models::find_by_name(model_name)
            .filter(|model| model.engine_type == engine_type)
            .ok_or_else(|| format!("Unknown {engine_type} model: {model_name}"))?;
        if definition.built_in {
            return Err(format!(
                "Model '{}' is built into the system and cannot be deleted",
                model_name
            ));
        }

        let model_subdir = self.models_dir.join(model_name);

        if !model_subdir.exists() {
            return Err(format!(
                "Model '{}' not found at path: {}",
                model_name,
                model_subdir.display()
            ));
        }

        if definition.engine_type == EngineType::Mlx {
            let mut cache = self.engine_cache.lock().unwrap();
            if let Some(((_, cached_model, _, _), EngineInstance::Mlx(engine))) = cache.as_ref() {
                if cached_model == model_name {
                    engine.shutdown();
                    *cache = None;
                }
            }
        }

        std::fs::remove_dir_all(&model_subdir)
            .map_err(|e| format!("Failed to delete model '{}': {}", model_name, e))?;

        info!(
            engine = ?engine_type,
            model = %model_name,
            path = ?model_subdir,
            "model_deleted"
        );

        Ok(())
    }

    pub fn recommend_by_language(&self, lang: &str) -> Vec<RecommendedModel> {
        let recommended = models::recommend_by_language(lang);

        recommended
            .into_iter()
            .map(|model_def| {
                let downloaded = self.is_model_downloaded(model_def.engine_type, model_def.name);
                RecommendedModel {
                    engine_type: model_def.engine_type,
                    model_name: model_def.name.to_string(),
                    display_name: model_def.display_name.to_string(),
                    size_mb: model_def.size_mb as u64,
                    speed_score: model_def.speed_score,
                    accuracy_score: model_def.accuracy_score,
                    downloaded,
                }
            })
            .collect()
    }

    pub fn vad_model_path(&self) -> PathBuf {
        self.models_dir.join(SILERO_VAD_LOCAL_FILE)
    }

    pub fn is_vad_model_downloaded(&self) -> bool {
        downloaded_file_is_complete(&self.vad_model_path(), Some(1))
    }

    pub async fn download_vad_model(
        &self,
        cancel_flag: Arc<AtomicBool>,
    ) -> Result<PathBuf, String> {
        let output_path = self.vad_model_path();

        if downloaded_file_is_complete(&output_path, Some(1)) {
            info!("vad_model_already_exists");
            return Ok(output_path);
        }

        if output_path.exists() {
            info!("incomplete_vad_model_redownloading");
            remove_download_artifacts(&output_path);
        }

        info!(
            repo = SILERO_VAD_REPO,
            remote_file = SILERO_VAD_REMOTE_FILE,
            local_file = SILERO_VAD_LOCAL_FILE,
            "downloading_vad_model"
        );

        let source = HuggingFaceSource::new(SILERO_VAD_REPO, SILERO_VAD_REMOTE_FILE).into_source();
        let urls = source.urls();

        let options = DownloadOptions::new(&urls[0], &output_path)
            .with_fallbacks(urls[1..].to_vec())
            .with_cancel_flag(cancel_flag)
            .with_minimum_bytes(1)
            .with_model_name("silero-vad");

        let result = download(options).await?;

        info!(path = ?result.path, bytes = result.bytes, "vad_model_downloaded");
        Ok(result.path)
    }

    pub async fn ensure_vad_model(&self) -> Result<PathBuf, String> {
        if self.is_vad_model_downloaded() {
            return Ok(self.vad_model_path());
        }

        info!("vad_model_missing_auto_downloading");
        self.download_vad_model(Arc::new(AtomicBool::new(false)))
            .await
    }

    pub async fn ensure_default_model(&self, language: &str) -> Result<(), String> {
        let default = models::default_for_language(language);
        let engine_type = default.engine_type;
        let model_name = default.name;

        if self.is_model_downloaded(engine_type, model_name) {
            info!(model = model_name, "default_model_already_downloaded");
            return Ok(());
        }

        info!(
            engine = ?engine_type,
            model = model_name,
            language = language,
            "default_model_missing_auto_downloading"
        );

        self.download_model(
            engine_type,
            model_name,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            |_, _| {},
        )
        .await?;

        if let Err(e) = self.ensure_vad_model().await {
            tracing::warn!(error = %e, "vad_model_ensure_failed");
        }

        Ok(())
    }
}

fn extract_tar_bz2_archive(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    let output = Command::new("tar")
        .arg("-xjf")
        .arg(archive_path)
        .arg("-C")
        .arg(output_dir)
        .output()
        .map_err(|e| {
            format!(
                "Failed to run tar for Qwen3-ASR archive extraction: {}. \
                 Install a system tar/bsdtar command and retry.",
                e
            )
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!(
        "Qwen3-ASR archive extraction failed with status {}. stdout: {} stderr: {}",
        output.status,
        stdout.trim(),
        stderr.trim()
    ))
}

#[derive(Clone)]
pub(crate) enum EngineInstance {
    Local(SherpaOnnxEngine),
    Apple(AppleSpeechEngine),
    Mlx(MlxEngine),
}

impl EngineInstance {
    pub async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResult, String> {
        match self {
            EngineInstance::Local(engine) => engine.transcribe(request).await,
            EngineInstance::Apple(engine) => engine.transcribe(request).await,
            EngineInstance::Mlx(engine) => engine.transcribe(request).await,
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn initial_apple_catalogue_waits_for_background_availability() {
        let manager = UnifiedEngineManager::new(std::env::temp_dir());
        manager
            .apple_status_refreshing
            .store(true, std::sync::atomic::Ordering::Release);
        let query = manager.get_models_with_status(Some(EngineType::Apple));
        tokio::pin!(query);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut query)
                .await
                .is_err()
        );
        *manager.apple_status_cache.lock().unwrap() =
            Some(("auto".to_string(), AppleSpeechStatus::Ready));
        manager.apple_status_changed.notify_waiters();
        let models = tokio::time::timeout(std::time::Duration::from_secs(1), query)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(models.len(), 1);
        assert!(models[0].downloaded);
    }

    #[tokio::test]
    async fn resolved_catalogue_hides_confirmed_unsupported_apple_without_delaying_other_engines() {
        let manager = UnifiedEngineManager::new(std::env::temp_dir());
        *manager.apple_status_cache.lock().unwrap() =
            Some(("auto".to_string(), AppleSpeechStatus::OsUnsupported));
        assert!(manager
            .get_models_with_status(Some(EngineType::Apple))
            .await
            .unwrap()
            .is_empty());
        manager.apple_status_cache.lock().unwrap().take();
        manager
            .apple_status_refreshing
            .store(true, std::sync::atomic::Ordering::Release);
        let models = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            manager.get_models_with_status(Some(EngineType::Mlx)),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            models.len(),
            models::available()
                .iter()
                .filter(|model| model.engine_type == EngineType::Mlx)
                .count()
        );
    }

    #[tokio::test]
    async fn apple_availability_wait_is_bounded_and_can_be_retried() {
        let manager = UnifiedEngineManager::new(std::env::temp_dir());
        manager
            .apple_status_refreshing
            .store(true, std::sync::atomic::Ordering::Release);
        let error = manager
            .resolve_apple_status(std::time::Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(error.contains("timed out"));
        *manager.apple_status_cache.lock().unwrap() =
            Some(("auto".to_string(), AppleSpeechStatus::Ready));
        assert_eq!(
            manager
                .resolve_apple_status(std::time::Duration::from_millis(10))
                .await
                .unwrap(),
            AppleSpeechStatus::Ready
        );
    }

    #[tokio::test]
    async fn catalogue_wait_ignores_old_language_and_releases_all_waiters() {
        let manager = UnifiedEngineManager::new(std::env::temp_dir());
        *manager.dictation_language.lock().unwrap() = "fr".to_string();
        *manager.apple_status_cache.lock().unwrap() =
            Some(("en".to_string(), AppleSpeechStatus::OsUnsupported));
        manager
            .apple_status_refreshing
            .store(true, std::sync::atomic::Ordering::Release);
        let all_models = manager.get_models_with_status(None);
        let apple_models = manager.get_models_with_status(Some(EngineType::Apple));
        tokio::pin!(all_models, apple_models);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut all_models)
                .await
                .is_err()
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut apple_models)
                .await
                .is_err()
        );
        *manager.apple_status_cache.lock().unwrap() =
            Some(("fr".to_string(), AppleSpeechStatus::Ready));
        manager.apple_status_changed.notify_waiters();
        let (all_models, apple_models) =
            tokio::time::timeout(std::time::Duration::from_secs(1), async {
                tokio::join!(all_models, apple_models)
            })
            .await
            .unwrap();
        assert!(all_models
            .unwrap()
            .iter()
            .any(|model| model.name == models::APPLE_SPEECH.name && model.downloaded));
        assert_eq!(apple_models.unwrap().len(), 1);
    }

    #[test]
    fn platform_policy_apple_catalogue_preserves_real_readiness_states() {
        let directory = tempfile::tempdir().unwrap();
        let manager = UnifiedEngineManager::new(directory.path().join("models"));
        for status in [
            AppleSpeechStatus::OsUnsupported,
            AppleSpeechStatus::AssetsMissing,
            AppleSpeechStatus::LocaleUnsupported,
            AppleSpeechStatus::Ready,
        ] {
            *manager.apple_status_cache.lock().unwrap() = Some(("auto".to_string(), status));
            let models = manager.get_models(EngineType::Apple);
            assert_eq!(
                models.len(),
                usize::from(status != AppleSpeechStatus::OsUnsupported)
            );
            assert_eq!(
                manager.is_model_downloaded(EngineType::Apple, models::APPLE_SPEECH.name),
                status == AppleSpeechStatus::Ready
            );
            if let Some(model) = models.first() {
                assert!(model.built_in);
                assert_eq!(model.downloaded, status == AppleSpeechStatus::Ready);
                assert_eq!(
                    model.downloadable,
                    status == AppleSpeechStatus::AssetsMissing
                );
            }
        }
        assert!(matches!(
            manager.get_or_create_engine(EngineType::Apple, models::APPLE_SPEECH.name, Some("fr")),
            Ok(EngineInstance::Apple(_))
        ));
    }

    #[test]
    fn platform_policy_rejects_excluded_stt_even_when_cached() {
        let directory = tempfile::tempdir().unwrap();
        let manager = UnifiedEngineManager::new(directory.path().join("models"));
        *manager.engine_cache.lock().unwrap() = Some((
            (
                EngineType::Whisper,
                models::WHISPER_BASE.name.to_string(),
                None,
                InferenceProvider::Cpu,
            ),
            EngineInstance::Apple(AppleSpeechEngine::new(None)),
        ));
        let result =
            manager.get_or_create_engine(EngineType::Whisper, models::WHISPER_BASE.name, None);
        if mlx::runtime::supported() {
            assert!(result
                .err()
                .unwrap()
                .contains("not available on this computer"));
            assert!(!manager.is_model_loaded(models::WHISPER_BASE.name));
        } else {
            assert!(result.is_ok());
            assert!(manager.is_model_loaded(models::WHISPER_BASE.name));
        }
    }

    #[test]
    fn model_catalog_exposes_language_restrictions() {
        let manager =
            super::UnifiedEngineManager::new(std::env::temp_dir().join("catalog_metadata"));
        let sensevoice = manager.model_info(&models::SENSE_VOICE_SMALL);
        let whisper = manager.model_info(&models::WHISPER_BASE);
        assert_eq!(
            serde_json::to_value(sensevoice).unwrap()["language_codes"],
            serde_json::json!(["zh", "yue", "ja", "ko", "en"])
        );
        assert_eq!(
            serde_json::to_value(whisper).unwrap()["language_codes"],
            serde_json::json!([])
        );
    }

    use super::*;

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn manager_with_cached_mlx(root: &Path) -> UnifiedEngineManager {
        let models_dir = root.join("models");
        std::fs::create_dir_all(models_dir.join("mlx-whisper-turbo")).unwrap();
        let python = mlx::runtime::python_path(&models_dir);
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        std::fs::write(python, "unused fixture").unwrap();
        std::fs::write(
            mlx::runtime::runtime_dir(&models_dir).join(".runtime-ready"),
            concat!("python=3.12.15\n", include_str!("mlx/requirements.lock")),
        )
        .unwrap();
        let engine = MlxEngine::new(&models_dir, "mlx-whisper-turbo").unwrap();
        let manager = UnifiedEngineManager::new(models_dir);
        *manager.engine_cache.lock().unwrap() = Some((
            (
                EngineType::Mlx,
                "mlx-whisper-turbo".to_string(),
                None,
                InferenceProvider::Cpu,
            ),
            EngineInstance::Mlx(engine),
        ));
        manager
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn language_change_preserves_cached_mlx_engine() {
        let directory = tempfile::tempdir().unwrap();
        let manager = manager_with_cached_mlx(directory.path());

        manager.clear_language_dependent_cache();

        assert!(manager.engine_cache.lock().unwrap().is_some());
    }

    #[test]
    fn language_change_invalidates_native_engine_cache() {
        let directory = tempfile::tempdir().unwrap();
        let manager = UnifiedEngineManager::new(directory.path().join("models"));
        *manager.engine_cache.lock().unwrap() = Some((
            (
                EngineType::Apple,
                models::APPLE_SPEECH.name.to_string(),
                Some("fr".to_string()),
                InferenceProvider::Cpu,
            ),
            EngineInstance::Apple(AppleSpeechEngine::new(Some("fr"))),
        ));

        manager.clear_language_dependent_cache();

        assert!(manager.engine_cache.lock().unwrap().is_none());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn deleting_another_mlx_model_preserves_cached_engine() {
        let directory = tempfile::tempdir().unwrap();
        let manager = manager_with_cached_mlx(directory.path());
        let unused = manager.models_dir.join("mlx-parakeet-v3");
        std::fs::create_dir_all(&unused).unwrap();

        manager
            .delete_model(EngineType::Mlx, "mlx-parakeet-v3")
            .unwrap();

        assert!(!unused.exists());
        assert!(manager.engine_cache.lock().unwrap().is_some());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rejected_mlx_deletion_preserves_cached_engine() {
        for model in ["mlx-parakeet-v3", "mlx-whisper-turbo"] {
            let directory = tempfile::tempdir().unwrap();
            let manager = manager_with_cached_mlx(directory.path());
            std::fs::remove_dir_all(manager.models_dir.join("mlx-whisper-turbo")).unwrap();

            assert!(manager.delete_model(EngineType::Mlx, model).is_err());
            assert!(manager.engine_cache.lock().unwrap().is_some());
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn deleting_cached_mlx_model_unloads_it() {
        let directory = tempfile::tempdir().unwrap();
        let manager = manager_with_cached_mlx(directory.path());

        manager
            .delete_model(EngineType::Mlx, "mlx-whisper-turbo")
            .unwrap();

        assert!(manager.engine_cache.lock().unwrap().is_none());
    }

    #[test]
    fn test_engine_type_parsing() {
        assert_eq!(
            "whisper".parse::<EngineType>().unwrap(),
            EngineType::Whisper
        );
        assert_eq!(
            "sensevoice".parse::<EngineType>().unwrap(),
            EngineType::SenseVoice
        );
        assert_eq!(
            "WHISPER".parse::<EngineType>().unwrap(),
            EngineType::Whisper
        );
        assert_eq!(
            "qwen3-asr".parse::<EngineType>().unwrap(),
            EngineType::Qwen3Asr
        );
        assert!("unknown".parse::<EngineType>().is_err());
    }

    #[test]
    fn test_available_engines() {
        let engines = UnifiedEngineManager::available_engines();
        let expected = if mlx::runtime::supported() {
            vec![EngineType::Mlx, EngineType::Apple, EngineType::Cloud]
        } else {
            vec![
                EngineType::Whisper,
                EngineType::SenseVoice,
                EngineType::Qwen3Asr,
                EngineType::Apple,
                EngineType::Cloud,
            ]
        };
        assert_eq!(engines, expected);
        assert!(engines.contains(&EngineType::Cloud));
    }

    #[test]
    fn test_model_definitions() {
        assert_eq!(models::ALL.len(), 12);
        assert_eq!(models::SENSE_VOICE_SMALL.name, "sense-voice-small");
        assert_eq!(models::WHISPER_TINY.name, "whisper-tiny");
        assert_eq!(models::WHISPER_BASE.name, "whisper-base");
        assert_eq!(models::WHISPER_SMALL.name, "whisper-small");
        assert_eq!(models::WHISPER_MEDIUM.name, "whisper-medium");
        assert_eq!(models::WHISPER_LARGE_V3.name, "whisper-large-v3");
        assert_eq!(models::WHISPER_TURBO.name, "whisper-turbo");
        assert_eq!(models::QWEN3_ASR_0_6B_INT8.name, "qwen3-asr-0.6b-int8");
    }

    #[test]
    fn test_get_engine_by_model_name() {
        assert_eq!(
            UnifiedEngineManager::get_engine_by_model_name("sense-voice-small"),
            Some(EngineType::SenseVoice)
        );
        assert_eq!(
            UnifiedEngineManager::get_engine_by_model_name("whisper-base"),
            Some(EngineType::Whisper)
        );
        assert_eq!(
            UnifiedEngineManager::get_engine_by_model_name("whisper-small"),
            Some(EngineType::Whisper)
        );
        assert_eq!(
            UnifiedEngineManager::get_engine_by_model_name("qwen3-asr-0.6b-int8"),
            Some(EngineType::Qwen3Asr)
        );
        assert_eq!(
            UnifiedEngineManager::get_engine_by_model_name("cloud"),
            Some(EngineType::Cloud)
        );
        assert_eq!(
            UnifiedEngineManager::get_engine_by_model_name("unknown"),
            None
        );
    }

    #[test]
    fn test_recommend_by_language_zh() {
        let temp_dir = std::env::temp_dir().join("test_recommend_zh_v2");
        let _ = std::fs::create_dir_all(&temp_dir);
        let manager = UnifiedEngineManager::new(temp_dir.clone());

        let recommendations = manager.recommend_by_language("zh");
        assert!(!recommendations.is_empty());

        let has_sensevoice = recommendations
            .iter()
            .any(|r| r.engine_type == EngineType::SenseVoice);
        assert_eq!(has_sensevoice, !mlx::runtime::supported());
        assert_eq!(
            recommendations
                .iter()
                .all(|model| model.engine_type == EngineType::Mlx),
            mlx::runtime::supported()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_recommend_by_language_en() {
        let temp_dir = std::env::temp_dir().join("test_recommend_en_v2");
        let _ = std::fs::create_dir_all(&temp_dir);
        let manager = UnifiedEngineManager::new(temp_dir.clone());

        let recommendations = manager.recommend_by_language("en");
        assert!(!recommendations.is_empty());

        // English is SenseVoice-preferred (zh/yue/ja/ko/en)
        let has_sensevoice = recommendations
            .iter()
            .any(|r| r.engine_type == EngineType::SenseVoice);
        assert_eq!(has_sensevoice, !mlx::runtime::supported());
        assert_eq!(
            recommendations
                .iter()
                .all(|model| model.engine_type == EngineType::Mlx),
            mlx::runtime::supported()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_transcription_result_creation() {
        use super::super::traits::TranscriptionResult;

        let result = TranscriptionResult::new("test text".to_string(), EngineType::Whisper, 1000);
        assert_eq!(result.text, "test text");
        assert_eq!(result.engine, EngineType::Whisper);
        assert_eq!(result.total_ms, 1000);
        assert!(result.model_load_ms.is_none());

        let result_with_metrics = TranscriptionResult::with_metrics(
            "test text".to_string(),
            EngineType::Whisper,
            1000,
            Some(100),
            Some(200),
            Some(700),
        );
        assert_eq!(result_with_metrics.model_load_ms, Some(100));
        assert_eq!(result_with_metrics.preprocess_ms, Some(200));
        assert_eq!(result_with_metrics.inference_ms, Some(700));
    }

    #[test]
    fn test_load_model_not_downloaded() {
        let temp_dir = std::env::temp_dir().join("test_load_model_v2");
        let _ = std::fs::create_dir_all(&temp_dir);
        let manager = UnifiedEngineManager::new(temp_dir.clone());

        let result = manager.load_model(EngineType::Whisper, "whisper-base");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains(if mlx::runtime::supported() {
            "not available on this computer"
        } else {
            "not downloaded"
        }));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_delete_model_not_exists() {
        let temp_dir = std::env::temp_dir().join("test_delete_model_v2");
        let _ = std::fs::create_dir_all(&temp_dir);
        let manager = UnifiedEngineManager::new(temp_dir.clone());

        let result = manager.delete_model(EngineType::Whisper, "whisper-base");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_available_model_falls_back_to_downloaded() {
        let temp_dir = std::env::temp_dir().join("test_resolve_model_fallback");
        let _ = std::fs::create_dir_all(&temp_dir);

        write_sparse_model_files(&temp_dir, &models::SENSE_VOICE_SMALL);

        let manager = UnifiedEngineManager::new(temp_dir.clone());

        let (engine_type, model_name) = manager.resolve_available_model("whisper-base", "zh");
        let expected = if mlx::runtime::supported() {
            (EngineType::Mlx, "mlx-whisper-turbo")
        } else {
            (EngineType::SenseVoice, "sense-voice-small")
        };
        assert_eq!((engine_type, model_name.as_str()), expected);

        let (engine_type2, model_name2) =
            manager.resolve_available_model("sense-voice-small", "zh");
        assert_eq!((engine_type2, model_name2.as_str()), expected);

        let (engine_type3, model_name3) =
            manager.resolve_available_model("nonexistent-model", "auto");
        assert_eq!((engine_type3, model_name3.as_str()), expected);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_qwen3_asr_model_download_detection() {
        let temp_dir = std::env::temp_dir().join("test_qwen3_asr_model_downloaded");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::create_dir_all(&temp_dir);
        let manager = UnifiedEngineManager::new(temp_dir.clone());

        assert!(
            !manager.is_model_downloaded(EngineType::Qwen3Asr, models::QWEN3_ASR_0_6B_INT8.name)
        );

        write_sparse_model_files(&temp_dir, &models::QWEN3_ASR_0_6B_INT8);

        assert_eq!(
            manager.is_model_downloaded(EngineType::Qwen3Asr, models::QWEN3_ASR_0_6B_INT8.name),
            !mlx::runtime::supported()
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_resolve_available_model_nothing_downloaded() {
        let temp_dir = std::env::temp_dir().join("test_resolve_model_nothing");
        let _ = std::fs::create_dir_all(&temp_dir);
        let manager = UnifiedEngineManager::new(temp_dir.clone());

        let (_, model_name) = manager.resolve_available_model("whisper-base", "auto");
        assert_eq!(model_name, models::default_for_language("auto").name);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn apple_status_is_resolved_in_the_background() {
        let manager = UnifiedEngineManager::new(std::env::temp_dir());
        manager.set_dictation_language("fr");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while manager.apple_status_cache.lock().unwrap().is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "the background status query should answer"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let cached = manager.apple_status_cache.lock().unwrap().clone();
        assert_eq!(cached.map(|(language, _)| language), Some("fr".to_string()));
    }

    #[test]
    fn built_in_models_cannot_be_deleted() {
        let temp_dir = std::env::temp_dir().join("test_delete_built_in_model");
        let manager = UnifiedEngineManager::new(temp_dir);

        let error = manager
            .delete_model(EngineType::Apple, models::APPLE_SPEECH.name)
            .expect_err("a built-in model must not be deleted");

        assert!(error.contains("built into the system"));
    }

    #[test]
    fn fallback_model_skips_the_excluded_engine_and_built_in_models() {
        let temp_dir = std::env::temp_dir().join("test_fallback_model_excludes_engine");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let manager = UnifiedEngineManager::new(temp_dir.clone());
        write_sparse_model_files(&temp_dir, &models::WHISPER_BASE);

        assert_eq!(
            manager.fallback_model("fr", EngineType::Apple),
            if mlx::runtime::supported() {
                None
            } else {
                Some(models::WHISPER_BASE.name.to_string())
            }
        );
        assert_eq!(manager.fallback_model("fr", EngineType::Whisper), None);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn model_list_exposes_the_built_in_flag() {
        let temp_dir = std::env::temp_dir().join("test_model_list_built_in_flag");
        let manager = UnifiedEngineManager::new(temp_dir);

        for info in manager.get_all_models() {
            let definition = models::find_by_name(&info.name).expect("listed model is defined");
            assert_eq!(info.built_in, definition.built_in, "{}", info.name);
        }
    }

    fn write_sparse_model_files(base_dir: &std::path::Path, model: &models::ModelDefinition) {
        let model_dir = base_dir.join(model.name);
        std::fs::create_dir_all(&model_dir).unwrap();

        for file in model.files {
            let path = model_dir.join(file.filename);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            let handle = std::fs::File::create(&path).unwrap();
            handle.set_len(file.minimum_complete_bytes()).unwrap();
        }
    }
}
