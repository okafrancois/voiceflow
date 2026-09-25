use std::time::Instant;

use tracing::info;

use super::bridge::AppleSpeechSession;
use crate::stt_engine::traits::{EngineType, TranscriptionRequest, TranscriptionResult};

const SAMPLE_RATE: f64 = 16_000.0;
const MIN_AUDIO_SECS: f64 = 0.35;
/// Samples handed to the analyzer per call, like a recording would.
const FEED_CHUNK_SAMPLES: usize = 16_000;

/// Batch entry point for complete audio (file transcription, retry).
#[derive(Clone)]
pub struct AppleSpeechEngine {
    locale: String,
}

impl AppleSpeechEngine {
    pub fn new(locale: Option<&str>) -> Self {
        Self {
            locale: locale.unwrap_or("auto").to_string(),
        }
    }

    pub async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResult, String> {
        let started = Instant::now();
        let duration_secs = request.samples.len() as f64 / SAMPLE_RATE;
        if duration_secs < MIN_AUDIO_SECS {
            return Ok(TranscriptionResult::with_metrics(
                String::new(),
                EngineType::Apple,
                0,
                None,
                None,
                None,
            ));
        }

        let locale = self.locale.clone();
        let samples = to_pcm16(&request.samples);
        let text = tokio::task::spawn_blocking(move || {
            let session = AppleSpeechSession::start(&locale)?;
            for chunk in samples.chunks(FEED_CHUNK_SAMPLES) {
                session.feed(chunk);
            }
            session.finish()
        })
        .await
        .map_err(|e| format!("Apple speech task failed: {e}"))??;

        let total_ms = started.elapsed().as_millis() as u64;
        info!(
            engine = "apple",
            chars = text.len(),
            duration_secs,
            total_ms,
            "transcription_completed"
        );
        Ok(TranscriptionResult::with_metrics(
            text,
            EngineType::Apple,
            total_ms,
            None,
            None,
            Some(total_ms),
        ))
    }
}

fn to_pcm16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_samples_convert_to_clamped_pcm16() {
        assert_eq!(
            to_pcm16(&[0.0, 1.0, -1.0, 2.0, -2.0, 0.5]),
            vec![0, 32767, -32767, 32767, -32767, 16383]
        );
    }

    #[tokio::test]
    async fn too_short_audio_returns_empty_text_without_a_session() {
        let engine = AppleSpeechEngine::new(Some("fr"));
        let request = TranscriptionRequest::new(vec![0.1; 1_000]);

        let result = engine
            .transcribe(request)
            .await
            .expect("short audio should not fail");

        assert!(result.text.is_empty());
        assert_eq!(result.engine, EngineType::Apple);
    }
}
