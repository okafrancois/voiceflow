use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;

use super::{integrity, models, process::OwnedProcess, runtime};
use crate::polish_engine::local_http::{build_local_system_prompt, strip_think_block};
use crate::polish_engine::{
    PolishEngine, PolishEngineType, PolishPreviewUpdate, PolishRequest, PolishResult,
};

const WORKER: &str = include_str!("worker.py");
const MAX_BYTES: usize = 1024 * 1024;
const WORKER_START_TIMEOUT: Duration = Duration::from_secs(30);

struct Worker {
    process: OwnedProcess,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    diagnostics: JoinHandle<()>,
    next_id: u64,
    model: Option<String>,
}

impl Worker {
    async fn start(
        python: &Path,
        script: &Path,
        models_dir: &Path,
        timeout: Duration,
    ) -> Result<Self, String> {
        let mut command = runtime::isolated_command(python, models_dir);
        command
            .args(["-I", "-u"])
            .arg(script)
            .arg("--models-dir")
            .arg(models_dir)
            .arg("--parent-pid")
            .arg(std::process::id().to_string());
        let mut process = OwnedProcess::spawn(command)?;
        let input = process
            .child
            .stdin
            .take()
            .ok_or("managed polish input missing")?;
        let output = BufReader::new(
            process
                .child
                .stdout
                .take()
                .ok_or("managed polish output missing")?,
        );
        let mut stderr = process
            .child
            .stderr
            .take()
            .ok_or("managed polish diagnostics missing")?;
        let diagnostics = tokio::spawn(async move {
            let mut buffer = [0u8; 2048];
            loop {
                match stderr.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(count) => {
                        tracing::debug!(message = %String::from_utf8_lossy(&buffer[..count]), "mlx_polish_worker_diagnostic")
                    }
                    Err(error) => {
                        tracing::debug!(%error, "mlx_polish_worker_diagnostics_failed");
                        break;
                    }
                }
            }
        });
        let mut worker = Self {
            process,
            input,
            output,
            diagnostics,
            next_id: 0,
            model: None,
        };
        let ready = tokio::time::timeout(timeout, worker.receive())
            .await
            .map_err(|_| "managed polish worker readiness timed out".to_string())??;
        if ready["protocol"] != 1
            || ready["kind"] != "ready"
            || ready["mlx_lm_version"] != "0.32.2"
            || ready["mlx_version"] != "0.32.4"
        {
            return Err("incompatible managed polish worker readiness handshake".to_string());
        }
        Ok(worker)
    }

    async fn receive(&mut self) -> Result<Value, String> {
        let mut line = Vec::new();
        loop {
            let bytes = self
                .output
                .fill_buf()
                .await
                .map_err(|error| format!("cannot read managed polish response: {error}"))?;
            if bytes.is_empty() {
                return Err("managed polish worker exited before responding".to_string());
            }
            let count = bytes
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|offset| offset + 1)
                .unwrap_or(bytes.len());
            if line.len() + count > MAX_BYTES {
                return Err("managed polish response exceeds protocol limit".to_string());
            }
            line.extend_from_slice(&bytes[..count]);
            self.output.consume(count);
            if line.last() == Some(&b'\n') {
                break;
            }
        }
        serde_json::from_slice(&line)
            .map_err(|error| format!("invalid managed polish worker response: {error}"))
    }

    async fn request(&mut self, mut request: Value) -> Result<Value, String> {
        self.next_id += 1;
        request["id"] = json!(self.next_id);
        let mut bytes = serde_json::to_vec(&request)
            .map_err(|error| format!("cannot encode managed polish request: {error}"))?;
        if bytes.len() > MAX_BYTES {
            return Err("managed polish request exceeds protocol limit".to_string());
        }
        bytes.push(b'\n');
        tokio::time::timeout(Duration::from_secs(5), async {
            self.input.write_all(&bytes).await?;
            self.input.flush().await
        })
        .await
        .map_err(|_| "managed polish input timed out".to_string())?
        .map_err(|error| format!("cannot send managed polish request: {error}"))?;
        let response = self.receive().await?;
        if response["id"].as_u64() != Some(self.next_id) {
            return Err("managed polish returned a mismatched request ID".to_string());
        }
        match response["ok"].as_bool() {
            Some(true) => Ok(response),
            Some(false) => Err(format!(
                "managed polish inference failed: {}",
                response["error"].as_str().unwrap_or("unknown worker error")
            )),
            None => Err("managed polish response is missing success status".to_string()),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.diagnostics.abort();
    }
}

pub struct MlxPolishEngine {
    models_dir: PathBuf,
    worker: Mutex<Option<Worker>>,
    generation: watch::Sender<u64>,
    selected: StdMutex<Option<String>>,
    last_used: StdMutex<Instant>,
    integrity: integrity::ModelIntegrity,
}

impl MlxPolishEngine {
    pub fn new(models_dir: PathBuf) -> Self {
        let (generation, _) = watch::channel(0);
        Self {
            models_dir,
            worker: Mutex::new(None),
            generation,
            selected: StdMutex::new(None),
            last_used: StdMutex::new(Instant::now()),
            integrity: integrity::ModelIntegrity::default(),
        }
    }

    pub fn downloaded(&self, model: &str) -> bool {
        models::downloaded(&self.models_dir, model)
    }

    pub fn runtime_ready(&self) -> bool {
        runtime::installed(&self.models_dir)
    }

    pub fn is_loaded(&self, model: &str) -> bool {
        let Ok(mut guard) = self.worker.try_lock() else {
            return false;
        };
        let Some(worker) = guard.as_mut() else {
            return false;
        };
        match worker.process.child.try_wait() {
            Ok(None) => worker.model.as_deref() == Some(model),
            _ => {
                worker.process.reaped();
                *guard = None;
                false
            }
        }
    }

    pub fn stop(&self) {
        self.generation
            .send_modify(|generation| *generation = generation.wrapping_add(1));
        if let Ok(mut guard) = self.worker.try_lock() {
            *guard = None;
        }
        *self.selected.lock().unwrap() = None;
    }

    pub fn stop_model(&self, model: &str) {
        let selected = self.selected.lock().unwrap().as_deref() == Some(model);
        if selected {
            self.stop();
        }
    }

    pub fn stop_if_idle(&self, idle_for: Duration) -> bool {
        let Ok(mut guard) = self.worker.try_lock() else {
            return false;
        };
        if guard.is_none() || self.last_used.lock().unwrap().elapsed() < idle_for {
            return false;
        }
        *guard = None;
        *self.selected.lock().unwrap() = None;
        true
    }

    pub async fn preload(&self, model: &str) -> Result<(), String> {
        self.check_model(model)?;
        self.execute(
            json!({"action": "load", "model": model}),
            Duration::from_secs(120),
        )
        .await
        .map(|_| ())
    }

    fn check_model(&self, model: &str) -> Result<(), String> {
        runtime::ensure_supported()?;
        if !self.downloaded(model) {
            return Err("managed polish model or runtime is incomplete; download the selected model in Voice Flow".to_string());
        }
        Ok(())
    }

    async fn execute(&self, request: Value, timeout: Duration) -> Result<Value, String> {
        let model = request["model"]
            .as_str()
            .and_then(models::find)
            .ok_or("unknown managed polish checkpoint")?;
        self.execute_prepared(
            request,
            timeout,
            self.integrity.verify(
                &self.models_dir,
                model,
                Arc::new(AtomicBool::new(false)),
                integrity::VERIFY_TIMEOUT,
            ),
        )
        .await
    }

    async fn execute_prepared(
        &self,
        request: Value,
        timeout: Duration,
        preparation: impl Future<Output = Result<(), String>>,
    ) -> Result<Value, String> {
        let mut generation = self.generation.subscribe();
        let total_timeout = integrity::VERIFY_TIMEOUT
            .saturating_add(WORKER_START_TIMEOUT)
            .saturating_add(timeout);
        tokio::select! {
            biased;
            changed = generation.changed() => {
                changed.map_err(|error| format!("managed polish lifecycle ended: {error}"))?;
                Err("managed polish stopped".to_string())
            }
            result = tokio::time::timeout(total_timeout, self.execute_inner(request, timeout, preparation)) => {
                result.map_err(|_| "managed polish timed out; its owned process has been stopped".to_string())?
            }
        }
    }

    async fn execute_inner(
        &self,
        request: Value,
        timeout: Duration,
        preparation: impl Future<Output = Result<(), String>>,
    ) -> Result<Value, String> {
        let model = request["model"]
            .as_str()
            .ok_or("managed polish model missing")?;
        let (mut guard, previous) = tokio::time::timeout(integrity::VERIFY_TIMEOUT, async {
            let mut guard = self.worker.lock().await;
            *self.selected.lock().unwrap() = Some(model.to_string());
            let previous = guard.take();
            preparation.await?;
            Ok::<_, String>((guard, previous))
        })
        .await
        .map_err(|_| "managed polish checkpoint preparation timed out".to_string())??;
        let mut worker = match previous {
            Some(worker) => worker,
            None => {
                let started = tokio::time::timeout(WORKER_START_TIMEOUT, self.start_worker())
                    .await
                    .map_err(|_| "managed polish worker readiness timed out".to_string())
                    .and_then(|result| result);
                match started {
                    Ok(worker) => worker,
                    Err(error) => {
                        if let Err(invalidation_error) = runtime::invalidate(&self.models_dir) {
                            tracing::warn!(%invalidation_error, "mlx_polish_readiness_invalidation_failed");
                        }
                        return Err(format!(
                            "{error}; download the selected model to repair the polish runtime"
                        ));
                    }
                }
            }
        };
        let response = tokio::time::timeout(timeout, worker.request(request.clone()))
            .await
            .map_err(|_| {
                "managed polish timed out; its owned process has been stopped".to_string()
            })??;
        worker.model = Some(model.to_string());
        *self.last_used.lock().unwrap() = Instant::now();
        *guard = Some(worker);
        Ok(response)
    }

    async fn start_worker(&self) -> Result<Worker, String> {
        let directory = runtime::runtime_dir(&self.models_dir);
        let script = directory.join("voiceflow-polish-worker-v1.py");
        let temporary = directory.join(format!("worker-{}.tmp", uuid::Uuid::new_v4()));
        tokio::fs::write(&temporary, WORKER)
            .await
            .map_err(|error| format!("cannot prepare managed polish worker: {error}"))?;
        tokio::fs::rename(temporary, &script)
            .await
            .map_err(|error| format!("cannot install managed polish worker: {error}"))?;
        Worker::start(
            &runtime::python_path(&self.models_dir),
            &script,
            &self.models_dir,
            WORKER_START_TIMEOUT,
        )
        .await
    }
}

#[async_trait]
impl PolishEngine for MlxPolishEngine {
    fn engine_type(&self) -> PolishEngineType {
        PolishEngineType::Mlx
    }

    async fn polish(&self, request: PolishRequest) -> Result<PolishResult, String> {
        let started = Instant::now();
        let model = request
            .model_name
            .as_deref()
            .ok_or("managed polish model required")?;
        self.check_model(model)?;
        let prompt = build_local_system_prompt(&request.system_context, &request.language, false);
        let timeout = request.timeout.unwrap_or(Duration::from_secs(60));
        let max_tokens = request
            .text
            .chars()
            .count()
            .saturating_mul(2)
            .saturating_add(128)
            .clamp(256, 4096);
        let response = self.execute(json!({"action": "polish", "model": model, "text": request.text, "system_prompt": prompt, "max_tokens": max_tokens}), timeout).await?;
        let text = response["text"]
            .as_str()
            .ok_or("managed polish returned no text")?;
        let text = strip_think_block(text)
            .ok_or("managed polish returned an incomplete thinking block")?;
        if let Some(callback) = request.preview_callback {
            callback(PolishPreviewUpdate::final_text(text.clone()));
        }
        Ok(PolishResult::with_metrics(
            text,
            PolishEngineType::Mlx,
            started.elapsed().as_millis() as u64,
            response["model_load_ms"].as_u64(),
            response["inference_ms"].as_u64(),
        ))
    }
}

#[cfg(all(test, unix))]
#[path = "engine_tests.rs"]
mod tests;
