//! Local MLX Audio HTTP transcription behind the recording-consumer lifecycle.

use std::io::Cursor;
use std::net::IpAddr;
use std::time::Duration;

use reqwest::multipart::{Form, Part};
use reqwest::{Client, Url};
use serde::Deserialize;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::info;

use crate::commands::settings::CloudSttConfig;
use crate::stt_engine::traits::{PartialResult, PartialResultCallback, SttContext};

pub const MLX_AUDIO_BASE_URL: &str = "http://127.0.0.1:8000/v1";
pub const MLX_AUDIO_MODEL: &str = "mlx-community/whisper-large-v3-turbo-asr-fp16";

pub fn local_base_url(value: &str) -> Result<Url, String> {
    let value = if value.trim().is_empty() {
        MLX_AUDIO_BASE_URL
    } else {
        value.trim()
    };
    let mut url = Url::parse(value).map_err(|error| format!("Invalid MLX Audio URL: {error}"))?;
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    let local = host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !matches!(url.scheme(), "http" | "https")
        || !local
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("MLX Audio requires an HTTP(S) loopback URL on this computer, such as http://127.0.0.1:8000/v1".to_string());
    }
    let path = url.path().trim_end_matches('/');
    let path = if path.is_empty() {
        "/v1/".to_string()
    } else {
        format!("{path}/")
    };
    url.set_path(&path);
    Ok(url)
}

#[derive(Deserialize)]
struct Transcript {
    text: String,
}

pub struct MlxAudioClient {
    client: Client,
    base_url: Url,
    model: String,
    language: Option<String>,
    audio_tx: Mutex<Option<mpsc::Sender<Vec<i16>>>>,
    collector: Mutex<Option<JoinHandle<Vec<i16>>>>,
    on_partial: Option<PartialResultCallback>,
}

impl MlxAudioClient {
    pub fn new(
        config: CloudSttConfig,
        language: Option<&str>,
        _context: SttContext,
    ) -> Result<Self, String> {
        let base_url = local_base_url(&config.base_url)?;
        let model = if config.model.trim().is_empty() {
            MLX_AUDIO_MODEL.to_string()
        } else {
            config.model.trim().to_string()
        };
        let language = if config.language.trim().is_empty() {
            language.unwrap_or_default()
        } else {
            config.language.trim()
        };
        let language = (!language.is_empty() && language != "auto").then(|| {
            language
                .split(['-', '_'])
                .next()
                .unwrap_or(language)
                .to_lowercase()
        });
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|error| format!("Failed to create MLX Audio client: {error}"))?;
        Ok(Self {
            client,
            base_url,
            model,
            language,
            audio_tx: Mutex::new(None),
            collector: Mutex::new(None),
            on_partial: None,
        })
    }

    pub async fn connect(&mut self) -> Result<(), String> {
        if self.audio_tx.lock().await.is_some() {
            return Err("MLX Audio session is already connected".to_string());
        }
        let url = self
            .base_url
            .join("models")
            .map_err(|error| error.to_string())?;
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|error| {
                format!(
                    "Cannot connect to MLX Audio. Start the local MLX Audio server first: {error}"
                )
            })?;
        let body = response_body(response).await?;
        let models: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("Invalid MLX Audio models response: {error}"))?;
        if models.get("object").and_then(serde_json::Value::as_str) != Some("list")
            || !models.get("data").is_some_and(serde_json::Value::is_array)
        {
            return Err(
                "Invalid MLX Audio models response; expected an OpenAI-compatible model list"
                    .to_string(),
            );
        }
        if models["data"]
            .as_array()
            .is_some_and(|models| models.iter().any(|model| model["owned_by"] == "llamacpp"))
        {
            return Err("This address is the llama-server text-generation engine, not MLX Audio. Use a separate MLX Audio server address or select a managed MLX transcription model on Apple Silicon.".to_string());
        }
        // The server lists loaded models, not every downloaded checkpoint.
        let (sender, mut receiver) = mpsc::channel::<Vec<i16>>(32);
        let collector = tokio::spawn(async move {
            let mut samples = Vec::new();
            while let Some(chunk) = receiver.recv().await {
                samples.extend(chunk);
            }
            samples
        });
        *self.collector.lock().await = Some(collector);
        *self.audio_tx.lock().await = Some(sender);
        info!(provider = "mlx-audio", "local_stt_connected");
        Ok(())
    }

    pub async fn get_audio_sender(&self) -> Option<mpsc::Sender<Vec<i16>>> {
        self.audio_tx.lock().await.clone()
    }

    pub fn set_partial_callback(&mut self, callback: PartialResultCallback) {
        self.on_partial = Some(callback);
    }

    pub async fn finish(&self) -> Result<String, String> {
        drop(self.audio_tx.lock().await.take());
        let collector = self.collector.lock().await.take().ok_or_else(|| {
            "MLX Audio session is not connected or has already finished".to_string()
        })?;
        // Waiting for channel EOF preserves queued chunks and the final audio tail.
        let samples = collector
            .await
            .map_err(|error| format!("MLX Audio audio collection failed: {error}"))?;
        if samples.is_empty() {
            return Ok(String::new());
        }
        let sample_count = samples.len();
        let wav = encode_wav(&samples)?;
        let file = Part::bytes(wav)
            .file_name("recording.wav")
            .mime_str("audio/wav")
            .map_err(|error| error.to_string())?;
        let mut form = Form::new()
            .part("file", file)
            .text("model", self.model.clone())
            .text("response_format", "json");
        if let Some(language) = &self.language {
            form = form.text("language", language.clone());
        }
        let url = self
            .base_url
            .join("audio/transcriptions")
            .map_err(|error| error.to_string())?;
        let response = self
            .client
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|error| format!("MLX Audio transcription request failed: {error}"))?;
        let body = response_body(response).await?;
        let result: Transcript = serde_json::from_str(&body)
            .map_err(|error| format!("Invalid MLX Audio transcription response: {error}"))?;
        let text = result.text.trim().to_string();
        if let Some(callback) = &self.on_partial {
            callback(PartialResult {
                text: text.clone(),
                is_definite: true,
                is_final: true,
            });
        }
        info!(provider = "mlx-audio", sample_count, "local_stt_completed");
        Ok(text)
    }

    pub async fn close(&self) {
        drop(self.audio_tx.lock().await.take());
        if let Some(collector) = self.collector.lock().await.take() {
            collector.abort();
            let _ = collector.await;
        }
    }
}

fn encode_wav(samples: &[i16]) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    {
        let mut writer =
            hound::WavWriter::new(&mut output, spec).map_err(|error| error.to_string())?;
        for sample in samples {
            writer
                .write_sample(*sample)
                .map_err(|error| error.to_string())?;
        }
        writer.finalize().map_err(|error| error.to_string())?;
    }
    Ok(output.into_inner())
}

async fn response_body(response: reqwest::Response) -> Result<String, String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Failed to read MLX Audio response: {error}"))?;
    if !status.is_success() {
        return Err(format!("MLX Audio HTTP {status}: {body}"));
    }
    Ok(body)
}
