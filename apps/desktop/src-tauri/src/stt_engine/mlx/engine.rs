use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use super::runtime;
use crate::stt_engine::{EngineType, TranscriptionRequest, TranscriptionResult};

const WORKER_SOURCE: &str = include_str!("worker.py");
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    diagnostics: JoinHandle<()>,
    next_id: u64,
    loaded: bool,
}

impl Worker {
    async fn start(
        python: &Path,
        script: &Path,
        models_dir: &Path,
        audio_dir: &Path,
        timeout: Duration,
    ) -> Result<Self, String> {
        let mut child = Command::new(python)
            .args(["-I", "-u"])
            .arg(script)
            .arg("--models-dir")
            .arg(models_dir)
            .arg("--audio-dir")
            .arg(audio_dir)
            .arg("--parent-pid")
            .arg(std::process::id().to_string())
            .current_dir(models_dir)
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("Cannot start the managed MLX engine: {error}"))?;
        let input = child.stdin.take().ok_or("MLX worker has no input pipe")?;
        let output = BufReader::new(child.stdout.take().ok_or("MLX worker has no output pipe")?);
        let mut stderr = child
            .stderr
            .take()
            .ok_or("MLX worker has no diagnostic pipe")?;
        let diagnostics = tokio::spawn(async move {
            let mut buffer = [0u8; 2048];
            loop {
                match stderr.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(count) => {
                        debug!(message = %String::from_utf8_lossy(&buffer[..count]), "mlx_worker_diagnostic");
                    }
                    Err(error) => {
                        debug!(%error, "mlx_worker_diagnostic_read_failed");
                        break;
                    }
                }
            }
        });
        let mut worker = Self {
            child,
            input,
            output,
            diagnostics,
            next_id: 0,
            loaded: false,
        };
        let ready = worker.receive(timeout).await?;
        if ready["protocol"] != 1
            || ready["kind"] != "ready"
            || ready["mlx_audio_version"] != "0.6.1"
        {
            return Err(
                "The managed MLX engine returned an incompatible readiness handshake".to_string(),
            );
        }
        info!(process_id = ?worker.child.id(), "mlx_runtime_ready");
        Ok(worker)
    }

    async fn receive(&mut self, timeout: Duration) -> Result<Value, String> {
        let read = async {
            let mut line = Vec::new();
            loop {
                let available = self
                    .output
                    .fill_buf()
                    .await
                    .map_err(|error| format!("Cannot read MLX worker response: {error}"))?;
                if available.is_empty() {
                    return Err(
                        "The managed MLX engine exited before returning a response".to_string()
                    );
                }
                let count = available
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(available.len(), |position| position + 1);
                if line.len() + count > MAX_RESPONSE_BYTES {
                    return Err("MLX worker response exceeded the protocol size limit".to_string());
                }
                let complete = available[count - 1] == b'\n';
                line.extend_from_slice(&available[..count]);
                self.output.consume(count);
                if complete {
                    return serde_json::from_slice(&line)
                        .map_err(|error| format!("Invalid MLX worker response: {error}"));
                }
            }
        };
        tokio::time::timeout(timeout, read).await.map_err(|_| {
            "The managed MLX engine timed out; its process has been stopped".to_string()
        })?
    }

    async fn request(&mut self, mut request: Value) -> Result<Value, String> {
        self.next_id += 1;
        request["id"] = json!(self.next_id);
        let mut bytes = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("MLX worker request exceeded the protocol size limit".to_string());
        }
        bytes.push(b'\n');
        tokio::time::timeout(Duration::from_secs(5), async {
            self.input.write_all(&bytes).await?;
            self.input.flush().await
        })
        .await
        .map_err(|_| "MLX worker input timed out".to_string())?
        .map_err(|error| format!("Cannot send a request to MLX: {error}"))?;
        let response = self.receive(REQUEST_TIMEOUT).await?;
        if response["id"].as_u64() != Some(self.next_id) {
            return Err("MLX worker returned a response for a different request".to_string());
        }
        match response["ok"].as_bool() {
            Some(true) => Ok(response),
            Some(false) => Err(format!(
                "MLX inference failed: {}",
                response["error"].as_str().unwrap_or("unknown worker error")
            )),
            None => Err("MLX worker response is missing its success status".to_string()),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Err(error) = self.child.start_kill() {
            warn!(%error, "mlx_worker_stop_failed");
        }
        self.diagnostics.abort();
    }
}

#[derive(Clone)]
pub struct MlxEngine {
    models_dir: PathBuf,
    model: String,
    worker: Arc<Mutex<Option<Worker>>>,
    loaded: Arc<AtomicBool>,
    shutdown: watch::Sender<bool>,
}

impl MlxEngine {
    pub fn new(models_dir: &Path, model: &str) -> Result<Self, String> {
        if !runtime::supported() {
            return Err("MLX transcription requires an Apple Silicon Mac".to_string());
        }
        if super::models::revision(model).is_none() {
            return Err(format!("Unknown managed MLX model: {model}"));
        }
        if !runtime::installed(models_dir) {
            return Err("The managed MLX runtime is not installed. Download the selected model in Voice Flow to install it.".to_string());
        }
        let (shutdown, _) = watch::channel(false);
        Ok(Self {
            models_dir: models_dir.to_path_buf(),
            model: model.to_string(),
            worker: Arc::new(Mutex::new(None)),
            loaded: Arc::new(AtomicBool::new(false)),
            shutdown,
        })
    }

    pub fn is_loaded(&self) -> bool {
        if *self.shutdown.borrow() {
            return false;
        }
        if let Ok(mut guard) = self.worker.try_lock() {
            let alive = guard
                .as_mut()
                .is_some_and(|worker| worker.child.try_wait().is_ok_and(|status| status.is_none()));
            self.loaded.store(alive, Ordering::Release);
        }
        self.loaded.load(Ordering::Acquire)
    }

    async fn execute(&self, request: Option<Value>) -> Result<Value, String> {
        let mut shutdown = self.shutdown.subscribe();
        if *shutdown.borrow() {
            return Err("The managed MLX engine has been stopped".to_string());
        }
        tokio::select! {
            biased;
            _ = shutdown.wait_for(|stopped| *stopped) => {
                self.loaded.store(false, Ordering::Release);
                Err("The managed MLX engine has been stopped".to_string())
            }
            response = self.execute_inner(request) => response,
        }
    }

    pub fn shutdown(&self) {
        self.shutdown.send_replace(true);
        self.loaded.store(false, Ordering::Release);
        if let Ok(mut worker) = self.worker.try_lock() {
            *worker = None;
        }
    }

    async fn execute_inner(&self, request: Option<Value>) -> Result<Value, String> {
        let mut guard = self.worker.lock().await;
        self.loaded.store(false, Ordering::Release);
        let mut worker = match guard.take() {
            Some(worker) => worker,
            None => {
                let directory = runtime::runtime_dir(&self.models_dir);
                let audio_dir = directory.join("audio");
                tokio::fs::create_dir_all(&audio_dir)
                    .await
                    .map_err(|error| format!("Cannot create MLX audio directory: {error}"))?;
                let script = directory.join("voiceflow-worker-v1.py");
                let temporary = directory.join(format!("worker-{}.tmp", uuid::Uuid::new_v4()));
                tokio::fs::write(&temporary, WORKER_SOURCE)
                    .await
                    .map_err(|error| format!("Cannot prepare MLX worker: {error}"))?;
                tokio::fs::rename(&temporary, &script)
                    .await
                    .map_err(|error| format!("Cannot install MLX worker: {error}"))?;
                match Worker::start(
                    &runtime::python_path(&self.models_dir),
                    &script,
                    &self.models_dir,
                    &audio_dir,
                    Duration::from_secs(30),
                )
                .await
                {
                    Ok(worker) => worker,
                    Err(error) => {
                        if let Err(invalidation_error) = runtime::invalidate(&self.models_dir) {
                            warn!(%invalidation_error, "mlx_runtime_invalidation_failed");
                        }
                        return Err(format!("{error}. Download the selected model in Voice Flow to repair its MLX runtime."));
                    }
                }
            }
        };
        let result = async {
            let loaded = if worker.loaded {
                json!({"model_load_ms": 0})
            } else {
                let result = worker
                    .request(json!({"action": "load", "model": self.model}))
                    .await?;
                worker.loaded = true;
                result
            };
            self.loaded.store(true, Ordering::Release);
            match request {
                Some(request) => {
                    let mut result = worker.request(request).await?;
                    result["model_load_ms"] = loaded["model_load_ms"].clone();
                    Ok(result)
                }
                None => Ok(loaded),
            }
        }
        .await;
        match result {
            Ok(response) => {
                *guard = Some(worker);
                Ok(response)
            }
            Err(error) => {
                self.loaded.store(false, Ordering::Release);
                Err(error)
            }
        }
    }

    pub async fn preload(&self) -> Result<(), String> {
        self.execute(None).await.map(|_| ())
    }

    pub async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResult, String> {
        let started = Instant::now();
        if request.samples.is_empty() {
            return Ok(TranscriptionResult::new(String::new(), EngineType::Mlx, 0));
        }
        let audio_dir = runtime::runtime_dir(&self.models_dir).join("audio");
        let samples = request.samples;
        let audio = tokio::task::spawn_blocking(move || AudioFile::create(&audio_dir, &samples))
            .await
            .map_err(|error| format!("Cannot prepare MLX recording: {error}"))??;
        let response = self
            .execute(Some(json!({
                "action": "transcribe", "model": self.model, "audio": audio.0,
                "language": request.language, "initial_prompt": request.initial_prompt,
            })))
            .await?;
        let text = response["text"]
            .as_str()
            .ok_or("MLX returned a response without transcription text")?
            .trim()
            .to_string();
        info!(model = %self.model, text_len = text.len(), "mlx_transcription_completed");
        Ok(TranscriptionResult::with_metrics(
            text,
            EngineType::Mlx,
            started.elapsed().as_millis() as u64,
            response["model_load_ms"].as_u64(),
            Some(0),
            response["inference_ms"].as_u64(),
        ))
    }
}

struct AudioFile(PathBuf);

impl AudioFile {
    fn create(directory: &Path, samples: &[f32]) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
        }
        let path = directory.join(format!("{}.wav", uuid::Uuid::new_v4()));
        let audio = Self(path.clone());
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(|error| error.to_string())?;
        let mut writer = hound::WavWriter::new(
            file,
            hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .map_err(|error| error.to_string())?;
        for sample in samples {
            if !sample.is_finite() {
                return Err("The recording contains invalid audio samples".to_string());
            }
            writer
                .write_sample((sample.clamp(-1.0, 1.0) * 32768.0).clamp(-32768.0, 32767.0) as i16)
                .map_err(|error| error.to_string())?;
        }
        writer.finalize().map_err(|error| error.to_string())?;
        Ok(audio)
    }
}

impl Drop for AudioFile {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                warn!(%error, "mlx_private_audio_cleanup_failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_wav_preserves_the_complete_audio_tail_and_is_removed() {
        let directory = tempfile::tempdir().unwrap();
        let samples = [-1.0, 0.0, 0.5, 32767.0 / 32768.0, -0.25];
        let audio = AudioFile::create(directory.path(), &samples).unwrap();
        let path = audio.0.clone();
        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, 16_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(
            reader
                .into_samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [-32768, 0, 16384, 32767, -8192]
        );
        drop(audio);
        assert!(!path.exists());
    }

    #[cfg(unix)]
    async fn fake_worker(
        source: &str,
        timeout: Duration,
    ) -> (tempfile::TempDir, Result<Worker, String>) {
        let directory = tempfile::tempdir().unwrap();
        let script = directory.path().join("fake_worker.py");
        std::fs::write(&script, source).unwrap();
        let worker = Worker::start(
            Path::new("/usr/bin/python3"),
            &script,
            directory.path(),
            directory.path(),
            timeout,
        )
        .await;
        (directory, worker)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn worker_rejects_wrong_readiness_protocol() {
        let (_directory, result) = fake_worker("print('{\"protocol\":2,\"kind\":\"ready\",\"mlx_audio_version\":\"0.6.1\"}', flush=True)\n", Duration::from_secs(5)).await;
        assert!(result.err().unwrap().contains("handshake"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn worker_startup_timeout_stops_its_child() {
        let (directory, result) = fake_worker("import os, pathlib, time\npathlib.Path('worker.pid').write_text(str(os.getpid()))\ntime.sleep(60)\n", Duration::from_millis(500)).await;
        assert!(result.err().unwrap().contains("timed out"));
        let pid = std::fs::read_to_string(directory.path().join("worker.pid")).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let status = Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap();
        assert!(!status.success());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn worker_rejects_malformed_and_mismatched_responses() {
        for response in ["not JSON", "{\"id\":99,\"ok\":true}"] {
            let source = format!("import sys\nprint('{{\"protocol\":1,\"kind\":\"ready\",\"mlx_audio_version\":\"0.6.1\"}}', flush=True)\nsys.stdin.readline()\nprint({response:?}, flush=True)\n");
            let (_directory, result) = fake_worker(&source, Duration::from_secs(5)).await;
            let error = result
                .unwrap_or_else(|error| panic!("{error}"))
                .request(json!({"action":"load"}))
                .await
                .unwrap_err();
            assert!(error.contains("response"), "{error}");
        }
    }

    #[cfg(unix)]
    async fn engine_with_waiting_worker() -> (tempfile::TempDir, MlxEngine, u32) {
        let (directory, worker) = fake_worker("import pathlib, sys, time\nprint('{\"protocol\":1,\"kind\":\"ready\",\"mlx_audio_version\":\"0.6.1\"}', flush=True)\nsys.stdin.readline()\npathlib.Path('request.received').touch()\ntime.sleep(60)\n", Duration::from_secs(5)).await;
        let worker = worker.unwrap_or_else(|error| panic!("{error}"));
        let process_id = worker.child.id().unwrap();
        let (shutdown, _) = tokio::sync::watch::channel(false);
        let engine = MlxEngine {
            models_dir: directory.path().to_path_buf(),
            model: "mlx-whisper-turbo".to_string(),
            worker: Arc::new(Mutex::new(Some(worker))),
            loaded: Arc::new(AtomicBool::new(false)),
            shutdown,
        };
        (directory, engine, process_id)
    }

    #[cfg(unix)]
    async fn wait_for_file(path: &Path) {
        for _ in 0..100 {
            if path.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("Worker did not receive the request");
    }

    #[cfg(unix)]
    async fn wait_for_process_id(path: &Path) -> u32 {
        for _ in 0..100 {
            match std::fs::read_to_string(path) {
                Ok(contents) => {
                    if contents.ends_with('\n') {
                        let process_id = contents.trim().parse().unwrap_or_else(|error| {
                            panic!("Invalid worker PID marker {}: {error}", path.display())
                        });
                        assert_ne!(process_id, 0, "Worker process ID must be positive");
                        return process_id;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    panic!(
                        "Could not read worker PID marker {}: {error}",
                        path.display()
                    );
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "Worker did not publish a complete process ID in {}",
            path.display()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_id_marker_waits_for_complete_contents() {
        let directory = tempfile::tempdir().unwrap();
        let process_file = directory.path().join("worker.pid");
        let pending_file = process_file.clone();
        let mut process_id = tokio::spawn(async move { wait_for_process_id(&pending_file).await });
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut process_id)
                .await
                .is_err()
        );
        std::fs::write(&process_file, "").unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut process_id)
                .await
                .is_err()
        );
        std::fs::write(&process_file, "12").unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut process_id)
                .await
                .is_err()
        );
        std::fs::write(&process_file, "12345\n").unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), process_id)
                .await
                .unwrap()
                .unwrap(),
            12345
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    #[should_panic(expected = "Invalid worker PID marker")]
    async fn process_id_marker_rejects_malformed_complete_contents() {
        let directory = tempfile::tempdir().unwrap();
        let process_file = directory.path().join("worker.pid");
        std::fs::write(&process_file, "invalid\n").unwrap();
        wait_for_process_id(&process_file).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    #[should_panic(expected = "Could not read worker PID marker")]
    async fn process_id_marker_reports_unexpected_read_errors() {
        let directory = tempfile::tempdir().unwrap();
        wait_for_process_id(directory.path()).await;
    }

    #[cfg(unix)]
    async fn assert_process_stopped(process_id: u32) {
        for _ in 0..100 {
            let alive = Command::new("/bin/kill")
                .args(["-0", &process_id.to_string()])
                .stderr(Stdio::null())
                .status()
                .await
                .unwrap()
                .success();
            if !alive {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("Owned worker {process_id} is still alive");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unloading_stops_an_active_request_even_when_the_engine_is_cloned() {
        let (directory, engine, process_id) = engine_with_waiting_worker().await;
        let active_engine = engine.clone();
        let active = tokio::spawn(async move { active_engine.preload().await });
        wait_for_file(&directory.path().join("request.received")).await;
        engine.shutdown();
        let error = tokio::time::timeout(Duration::from_secs(2), active)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(error.contains("stopped"), "{error}");
        assert!(!engine.is_loaded());
        assert_process_stopped(process_id).await;
        assert!(engine.preload().await.unwrap_err().contains("stopped"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancelling_an_active_request_stops_the_owned_worker() {
        let (directory, engine, process_id) = engine_with_waiting_worker().await;
        let active_engine = engine.clone();
        let active = tokio::spawn(async move { active_engine.preload().await });
        wait_for_file(&directory.path().join("request.received")).await;
        active.abort();
        assert!(active.await.unwrap_err().is_cancelled());
        assert!(!engine.is_loaded());
        assert_process_stopped(process_id).await;
    }

    #[cfg(unix)]
    fn engine_with_fake_runtime(directory: &Path, source: &str) -> MlxEngine {
        use std::os::unix::fs::PermissionsExt;

        let models_dir = directory.join("models");
        std::fs::create_dir_all(models_dir.join("mlx-whisper-turbo")).unwrap();
        std::fs::write(models_dir.join("mlx-whisper-turbo/config.json"), "{}").unwrap();
        let python = runtime::python_path(&models_dir);
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        std::fs::write(&python, format!("#!/bin/sh\n{source}\n")).unwrap();
        std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(
            runtime::runtime_dir(&models_dir).join(".runtime-ready"),
            "test marker",
        )
        .unwrap();
        let (shutdown, _) = watch::channel(false);
        MlxEngine {
            models_dir,
            model: "mlx-whisper-turbo".to_string(),
            worker: Arc::new(Mutex::new(None)),
            loaded: Arc::new(AtomicBool::new(false)),
            shutdown,
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_worker_startup_invalidates_readiness_but_preserves_runtime_and_models() {
        let directory = tempfile::tempdir().unwrap();
        let engine = engine_with_fake_runtime(directory.path(), "exit 1");
        assert!(engine.preload().await.is_err());
        assert!(!runtime::runtime_dir(&engine.models_dir)
            .join(".runtime-ready")
            .exists());
        assert!(runtime::python_path(&engine.models_dir).exists());
        assert!(engine
            .models_dir
            .join("mlx-whisper-turbo/config.json")
            .exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancelled_worker_startup_preserves_runtime_readiness() {
        let directory = tempfile::tempdir().unwrap();
        let engine =
            engine_with_fake_runtime(directory.path(), "echo $$ > worker.pid\nread pending");
        let active_engine = engine.clone();
        let active = tokio::spawn(async move { active_engine.preload().await });
        let process_file = engine.models_dir.join("worker.pid");
        let process_id = wait_for_process_id(&process_file).await;
        active.abort();
        assert!(active.await.unwrap_err().is_cancelled());
        assert!(runtime::runtime_dir(&engine.models_dir)
            .join(".runtime-ready")
            .exists());
        assert_process_stopped(process_id).await;
    }
}
