use crate::stt_engine::sherpa_onnx::engine::incremental_whisper_boundary;
use crate::stt_engine::traits::{
    EngineType, PartialResultCallback, RecordingConsumer, SttContext, TranscriptionRequest,
};
use crate::stt_engine::unified_manager::UnifiedEngineManager;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, instrument, warn};

const SAMPLE_RATE: f64 = 16_000.0;
const MIN_RECORDING_SECS: f64 = 0.35;

/// Decodes one window of 16 kHz mono samples into text.
#[async_trait]
pub trait WindowDecoder: Send + Sync + 'static {
    async fn decode(&self, samples: Vec<f32>) -> Result<String, String>;
}

/// Decodes windows through the shared local engine manager.
struct EngineWindowDecoder {
    engine_manager: Arc<UnifiedEngineManager>,
    engine_type: EngineType,
    model_name: String,
    language: String,
    prompt: Option<String>,
}

#[async_trait]
impl WindowDecoder for EngineWindowDecoder {
    async fn decode(&self, samples: Vec<f32>) -> Result<String, String> {
        let mut request = TranscriptionRequest::new(samples)
            .with_model(&self.model_name)
            .with_language(&self.language);
        if let Some(prompt) = &self.prompt {
            request = request.with_prompt(prompt);
        }

        let result = self
            .engine_manager
            .transcribe(self.engine_type, request)
            .await?;
        Ok(result.text)
    }
}

#[derive(Default)]
struct BufferState {
    pending: Vec<f32>,
    windows: Vec<JoinHandle<Result<String, String>>>,
    total_samples: usize,
}

/// Buffering consumer for local STT models (Whisper, SenseVoice, Qwen3-ASR via
/// sherpa-onnx).
///
/// Whisper recordings are decoded incrementally: each time the pending audio
/// can close a window (see `incremental_whisper_boundary`), that window starts
/// decoding while the recording continues, and `finish()` only decodes the
/// remaining audio. Other engines accumulate the whole recording and decode it
/// once in `finish()`. No partial results are produced.
pub struct BufferingConsumer {
    state: Mutex<BufferState>,
    decoder: Arc<dyn WindowDecoder>,
    incremental: bool,
}

impl BufferingConsumer {
    pub fn new(
        engine_manager: Arc<UnifiedEngineManager>,
        model_name: String,
        language: String,
        initial_prompt: Option<String>,
        stt_context: SttContext,
    ) -> Self {
        let engine_type = UnifiedEngineManager::get_engine_by_model_name(&model_name)
            .unwrap_or(EngineType::Whisper);

        let prompt_parts: Vec<&str> = [
            initial_prompt.as_deref(),
            stt_context.domain.as_deref(),
            stt_context.subdomain.as_deref(),
            stt_context.glossary.as_deref(),
        ]
        .iter()
        .filter_map(|&s| s.filter(|v| !v.is_empty()))
        .collect();
        let prompt = (!prompt_parts.is_empty()).then(|| prompt_parts.join(" "));

        let decoder = EngineWindowDecoder {
            engine_manager,
            engine_type,
            model_name,
            language,
            prompt,
        };

        Self::with_decoder(Arc::new(decoder), engine_type)
    }

    pub fn with_decoder(decoder: Arc<dyn WindowDecoder>, engine_type: EngineType) -> Self {
        Self {
            state: Mutex::new(BufferState::default()),
            decoder,
            incremental: engine_type == EngineType::Whisper,
        }
    }

    fn spawn_decode(&self, samples: Vec<f32>) -> JoinHandle<Result<String, String>> {
        let decoder = Arc::clone(&self.decoder);
        tokio::spawn(async move { decoder.decode(samples).await })
    }
}

#[async_trait]
impl RecordingConsumer for BufferingConsumer {
    async fn send_chunk(&self, pcm_data: Vec<i16>) -> Result<(), String> {
        let mut state = self.state.lock();
        state.total_samples += pcm_data.len();
        state
            .pending
            .extend(pcm_data.iter().map(|&s| s as f32 / 32768.0));

        if !self.incremental {
            return Ok(());
        }

        while let Some(cut) = incremental_whisper_boundary(&state.pending) {
            let remainder = state.pending.split_off(cut);
            let window = std::mem::replace(&mut state.pending, remainder);
            info!(
                window_index = state.windows.len(),
                window_secs = window.len() as f64 / SAMPLE_RATE,
                "incremental_window_decode_started"
            );
            let handle = self.spawn_decode(window);
            state.windows.push(handle);
        }

        Ok(())
    }

    #[instrument(skip(self), fields(incremental = self.incremental))]
    async fn finish(&self) -> Result<String, String> {
        let (pending, mut windows, total_samples) = {
            let mut state = self.state.lock();
            (
                std::mem::take(&mut state.pending),
                std::mem::take(&mut state.windows),
                state.total_samples,
            )
        };

        if total_samples == 0 {
            warn!("no_audio_chunks_buffered");
            return Ok(String::new());
        }

        let duration_secs = total_samples as f64 / SAMPLE_RATE;
        if duration_secs < MIN_RECORDING_SECS {
            info!(duration_secs, "recording_too_short-skipping");
            return Ok(String::new());
        }

        if !windows.is_empty() {
            info!(
                early_windows = windows.len(),
                final_window_secs = pending.len() as f64 / SAMPLE_RATE,
                "incremental_final_window_decode_started"
            );
        }
        if !pending.is_empty() {
            windows.push(self.spawn_decode(pending));
        }

        let mut texts = Vec::with_capacity(windows.len());
        for window in windows {
            let text = window
                .await
                .map_err(|e| format!("Transcription task failed: {e}"))??;
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                texts.push(trimmed.to_string());
            }
        }

        Ok(texts.join(" "))
    }

    fn set_partial_callback(&mut self, _callback: PartialResultCallback) {
        // No-op: batch engines don't produce partial results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stt_engine::traits::EngineType;
    use std::collections::VecDeque;
    use std::time::Duration;

    const SAMPLE_RATE: usize = 16_000;
    const CHUNK_SAMPLES: usize = SAMPLE_RATE / 2;

    type Reply = (Duration, Result<String, String>);

    #[derive(Default)]
    struct ScriptedDecoder {
        calls: Mutex<Vec<Vec<f32>>>,
        replies: Mutex<VecDeque<Reply>>,
    }

    impl ScriptedDecoder {
        fn with_replies(replies: Vec<Reply>) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                replies: Mutex::new(replies.into()),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.lock().len()
        }

        fn decoded_samples(&self) -> Vec<Vec<i16>> {
            self.calls
                .lock()
                .iter()
                .map(|window| {
                    window
                        .iter()
                        .map(|sample| (sample * 32768.0).round() as i16)
                        .collect()
                })
                .collect()
        }
    }

    #[async_trait]
    impl WindowDecoder for ScriptedDecoder {
        async fn decode(&self, samples: Vec<f32>) -> Result<String, String> {
            let call_number = {
                let mut calls = self.calls.lock();
                calls.push(samples);
                calls.len()
            };
            let reply = self.replies.lock().pop_front();
            match reply {
                Some((delay, result)) => {
                    tokio::time::sleep(delay).await;
                    result
                }
                None => Ok(format!("w{call_number}")),
            }
        }
    }

    fn recording(seconds: usize) -> Vec<i16> {
        (0..SAMPLE_RATE * seconds)
            .map(|index| ((index % 20_000) as i16) - 10_000)
            .collect()
    }

    async fn feed(consumer: &BufferingConsumer, samples: &[i16]) {
        for chunk in samples.chunks(CHUNK_SAMPLES) {
            consumer
                .send_chunk(chunk.to_vec())
                .await
                .expect("sending a chunk should succeed");
        }
        // Let spawned window decodes reach the decoder.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    #[tokio::test]
    async fn short_whisper_recording_decodes_once_on_finish() {
        let decoder = Arc::new(ScriptedDecoder::default());
        let consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Whisper);
        let samples = recording(20);

        feed(&consumer, &samples).await;
        assert_eq!(decoder.call_count(), 0);

        let text = consumer.finish().await.expect("finish should succeed");

        assert_eq!(decoder.decoded_samples(), vec![samples]);
        assert_eq!(text, "w1");
    }

    #[tokio::test]
    async fn whisper_recording_at_the_threshold_is_decoded_once_on_finish() {
        let decoder = Arc::new(ScriptedDecoder::default());
        let consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Whisper);
        let samples = recording(33);

        feed(&consumer, &samples[..samples.len() - 1]).await;
        assert_eq!(decoder.call_count(), 0);
        feed(&consumer, &samples[samples.len() - 1..]).await;

        consumer.finish().await.expect("finish should succeed");

        let windows = decoder.decoded_samples();
        assert_eq!(windows.concat(), samples);
        assert!(windows.len() <= 2);
    }

    #[tokio::test]
    async fn long_whisper_recording_decodes_windows_while_recording() {
        let decoder = Arc::new(ScriptedDecoder::default());
        let consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Whisper);
        let samples = recording(90);

        feed(&consumer, &samples).await;
        let decoded_before_finish = decoder.call_count();

        let text = consumer.finish().await.expect("finish should succeed");

        let windows = decoder.decoded_samples();
        assert!(decoded_before_finish >= 2, "got {decoded_before_finish}");
        assert_eq!(windows.len(), decoded_before_finish + 1);
        assert_eq!(windows.concat(), samples);
        assert!(windows
            .iter()
            .all(|window| window.len() <= SAMPLE_RATE * 33));
        let expected: Vec<String> = (1..=windows.len()).map(|n| format!("w{n}")).collect();
        assert_eq!(text, expected.join(" "));
    }

    #[tokio::test]
    async fn window_texts_keep_recording_order_and_skip_empty_windows() {
        let decoder = Arc::new(ScriptedDecoder::with_replies(vec![
            (Duration::from_millis(50), Ok(" first ".to_string())),
            (Duration::ZERO, Ok(String::new())),
            (Duration::ZERO, Ok("last".to_string())),
        ]));
        let consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Whisper);

        feed(&consumer, &recording(70)).await;
        let text = consumer.finish().await.expect("finish should succeed");

        assert_eq!(decoder.call_count(), 3);
        assert_eq!(text, "first last");
    }

    #[tokio::test]
    async fn a_failed_window_fails_the_transcription() {
        let decoder = Arc::new(ScriptedDecoder::with_replies(vec![
            (Duration::ZERO, Err("window decode failed".to_string())),
            (Duration::ZERO, Ok("tail".to_string())),
        ]));
        let consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Whisper);

        feed(&consumer, &recording(40)).await;
        let error = consumer
            .finish()
            .await
            .expect_err("a failed window must fail the transcription");

        assert_eq!(error, "window decode failed");
    }

    #[tokio::test]
    async fn non_whisper_recordings_are_decoded_once_on_finish() {
        for engine_type in [EngineType::SenseVoice, EngineType::Qwen3Asr] {
            let decoder = Arc::new(ScriptedDecoder::default());
            let consumer = BufferingConsumer::with_decoder(decoder.clone(), engine_type);
            let samples = recording(90);

            feed(&consumer, &samples).await;
            assert_eq!(decoder.call_count(), 0);
            consumer.finish().await.expect("finish should succeed");

            assert_eq!(decoder.decoded_samples(), vec![samples]);
        }
    }

    /// Feeds a 16 kHz mono WAV (`VF_WHISPER_WAV`) in real time through the
    /// incremental consumer with the real whisper-turbo model from
    /// `VF_MODELS_DIR`, then decodes the same audio in one block, and prints
    /// the post-recording wait of both.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs whisper-turbo and a WAV file; run manually"]
    async fn real_whisper_incremental_decode_matches_a_single_decode() {
        let models_dir = std::env::var("VF_MODELS_DIR").expect("set VF_MODELS_DIR");
        let wav = std::env::var("VF_WHISPER_WAV").expect("set VF_WHISPER_WAV");
        let samples: Vec<i16> = hound::WavReader::open(wav)
            .expect("WAV should open")
            .into_samples::<i16>()
            .map(|sample| sample.expect("WAV sample should decode"))
            .collect();
        let manager = Arc::new(UnifiedEngineManager::new(models_dir.into()));
        manager.set_provider(true);
        manager
            .load_model(EngineType::Whisper, "whisper-turbo")
            .expect("whisper-turbo should load");

        let consumer = BufferingConsumer::new(
            manager.clone(),
            "whisper-turbo".to_string(),
            "fr".to_string(),
            None,
            SttContext::default(),
        );
        for chunk in samples.chunks(CHUNK_SAMPLES / 5) {
            consumer
                .send_chunk(chunk.to_vec())
                .await
                .expect("chunk should be accepted");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let started = std::time::Instant::now();
        let incremental = consumer.finish().await.expect("incremental decode");
        let incremental_wait = started.elapsed();

        let started = std::time::Instant::now();
        let request =
            TranscriptionRequest::new(samples.iter().map(|&s| s as f32 / 32768.0).collect())
                .with_model("whisper-turbo")
                .with_language("fr");
        let single = manager
            .transcribe(EngineType::Whisper, request)
            .await
            .expect("single decode")
            .text;
        let single_wait = started.elapsed();

        println!("incremental wait {incremental_wait:?}: {incremental}");
        println!("single-block wait {single_wait:?}: {single}");
        assert!(!incremental.trim().is_empty());
        assert!(incremental_wait < single_wait);
    }

    #[tokio::test]
    async fn empty_or_too_short_recordings_are_not_decoded() {
        let decoder = Arc::new(ScriptedDecoder::default());
        let consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Whisper);
        assert_eq!(consumer.finish().await, Ok(String::new()));

        let consumer = BufferingConsumer::with_decoder(decoder.clone(), EngineType::Whisper);
        feed(&consumer, &vec![100; SAMPLE_RATE / 5]).await;
        assert_eq!(consumer.finish().await, Ok(String::new()));

        assert_eq!(decoder.call_count(), 0);
    }
}
