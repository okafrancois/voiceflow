use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, Mutex};

use sherpa_onnx::{
    OfflineQwen3ASRModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineSenseVoiceModelConfig, OfflineWhisperModelConfig,
};
use tracing::{debug, info};

use crate::stt_engine::models::ModelDefinition;
use crate::stt_engine::traits::{EngineType, TranscriptionRequest, TranscriptionResult};
use crate::stt_engine::unified_manager::InferenceProvider;

struct ThreadSafeRecognizer(OfflineRecognizer);

const AUDIO_SAMPLE_RATE: usize = 16_000;
const WHISPER_MAX_SEGMENT_SECONDS: usize = 28;
const WHISPER_MAX_SEGMENT_SAMPLES: usize = AUDIO_SAMPLE_RATE * WHISPER_MAX_SEGMENT_SECONDS;
const WHISPER_MIN_SEGMENT_SAMPLES: usize = AUDIO_SAMPLE_RATE * 5;
const WHISPER_BOUNDARY_SEARCH_SAMPLES: usize = AUDIO_SAMPLE_RATE * 3;
const WHISPER_ENERGY_WINDOW_SAMPLES: usize = AUDIO_SAMPLE_RATE * 80 / 1_000;
const WHISPER_ENERGY_HOP_SAMPLES: usize = AUDIO_SAMPLE_RATE * 20 / 1_000;

// SAFETY: OfflineRecognizer wraps a C++ pointer (*const) that lacks
// auto-derived Send/Sync. We gate all access through a Mutex, so no
// concurrent mutation is possible. The underlying C++ recognizer is
// safe to use from any thread when accessed exclusively.
unsafe impl Send for ThreadSafeRecognizer {}
unsafe impl Sync for ThreadSafeRecognizer {}

fn transcription_segment_ranges(samples: &[f32], engine_type: EngineType) -> Vec<Range<usize>> {
    if samples.is_empty() {
        return Vec::new();
    }

    if engine_type != EngineType::Whisper || samples.len() <= WHISPER_MAX_SEGMENT_SAMPLES {
        return std::iter::once(0..samples.len()).collect();
    }

    let segment_count = samples.len().div_ceil(WHISPER_MAX_SEGMENT_SAMPLES);
    let mut ranges = Vec::with_capacity(segment_count);
    let mut segment_start: usize = 0;

    for boundary_number in 1..segment_count {
        let remaining_segments = segment_count - boundary_number;
        let ideal_boundary = samples.len() * boundary_number / segment_count;

        let earliest_boundary = segment_start
            .saturating_add(WHISPER_MIN_SEGMENT_SAMPLES)
            .max(
                samples
                    .len()
                    .saturating_sub(remaining_segments * WHISPER_MAX_SEGMENT_SAMPLES),
            );
        let latest_boundary = segment_start
            .saturating_add(WHISPER_MAX_SEGMENT_SAMPLES)
            .min(
                samples
                    .len()
                    .saturating_sub(remaining_segments * WHISPER_MIN_SEGMENT_SAMPLES),
            );

        let search_start = ideal_boundary
            .saturating_sub(WHISPER_BOUNDARY_SEARCH_SAMPLES)
            .max(earliest_boundary);
        let search_end = ideal_boundary
            .saturating_add(WHISPER_BOUNDARY_SEARCH_SAMPLES)
            .min(latest_boundary);
        let boundary = quietest_boundary(samples, search_start, search_end, ideal_boundary);

        ranges.push(segment_start..boundary);
        segment_start = boundary;
    }

    ranges.push(segment_start..samples.len());
    ranges
}

fn quietest_boundary(
    samples: &[f32],
    search_start: usize,
    search_end: usize,
    ideal_boundary: usize,
) -> usize {
    if search_start >= search_end {
        return ideal_boundary.clamp(search_start, search_end);
    }

    let mut best_boundary = ideal_boundary.clamp(search_start, search_end);
    let mut best_energy = boundary_energy(samples, best_boundary);
    let mut candidate = search_start;

    loop {
        let energy = boundary_energy(samples, candidate);
        let energy_order = energy.total_cmp(&best_energy);
        if energy_order.is_lt()
            || (energy_order.is_eq()
                && candidate.abs_diff(ideal_boundary) < best_boundary.abs_diff(ideal_boundary))
        {
            best_boundary = candidate;
            best_energy = energy;
        }

        if search_end - candidate < WHISPER_ENERGY_HOP_SAMPLES {
            break;
        }
        candidate += WHISPER_ENERGY_HOP_SAMPLES;
    }

    best_boundary
}

fn boundary_energy(samples: &[f32], boundary: usize) -> f64 {
    let half_window = WHISPER_ENERGY_WINDOW_SAMPLES / 2;
    let window_start = boundary.saturating_sub(half_window);
    let window_end = boundary.saturating_add(half_window).min(samples.len());

    samples[window_start..window_end]
        .iter()
        .map(|sample| f64::from(*sample) * f64::from(*sample))
        .sum::<f64>()
        / (window_end - window_start) as f64
}

fn decode_audio_with<F>(
    samples: &[f32],
    engine_type: EngineType,
    mut decode_segment: F,
) -> Result<String, String>
where
    F: FnMut(&[f32]) -> Result<String, String>,
{
    let ranges = transcription_segment_ranges(samples, engine_type);

    if ranges.len() > 1 {
        info!(
            engine = engine_type.as_str(),
            duration_secs = samples.len() as f64 / AUDIO_SAMPLE_RATE as f64,
            segment_count = ranges.len(),
            max_segment_secs = WHISPER_MAX_SEGMENT_SECONDS,
            "long_audio_segmented"
        );
    }

    let mut segment_texts = Vec::with_capacity(ranges.len());
    for (segment_index, range) in ranges.into_iter().enumerate() {
        debug!(
            engine = engine_type.as_str(),
            segment_index,
            segment_samples = range.len(),
            "audio_segment_decode_started"
        );
        let text = decode_segment(&samples[range])?;
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            segment_texts.push(trimmed.to_string());
        }
    }

    Ok(segment_texts.join(" "))
}

#[derive(Clone)]
pub struct SherpaOnnxEngine {
    recognizer: Arc<Mutex<ThreadSafeRecognizer>>,
    engine_type: EngineType,
}

impl SherpaOnnxEngine {
    /// Create a new engine from a model directory and model definition.
    ///
    /// The model directory should contain the model files as specified in the
    /// model definition's `files` array (e.g. `model.int8.onnx`, `tokens.txt`).
    pub fn new(
        model_dir: &Path,
        model_def: &ModelDefinition,
        language: Option<&str>,
        provider: InferenceProvider,
    ) -> Result<Self, String> {
        let model_subdir = model_dir.join(model_def.name);
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get().min(4) as i32)
            .unwrap_or(2);

        let mut config = OfflineRecognizerConfig::default();

        match model_def.engine_type {
            EngineType::SenseVoice => {
                let model_path = model_subdir.join("model.int8.onnx");
                let tokens_path = model_subdir.join("tokens.txt");

                if !model_path.exists() {
                    return Err(format!(
                        "SenseVoice model not found at: {}",
                        model_path.display()
                    ));
                }

                info!(
                    engine = "sensevoice",
                    model = %model_path.display(),
                    threads = num_threads,
                    "loading_model"
                );

                config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                    model: Some(
                        model_path
                            .to_str()
                            .ok_or("Invalid model path encoding")?
                            .to_string(),
                    ),
                    language: Some("auto".to_string()),
                    use_itn: true,
                };
                config.model_config.tokens = Some(
                    tokens_path
                        .to_str()
                        .ok_or("Invalid tokens path encoding")?
                        .to_string(),
                );
            }
            EngineType::Whisper => {
                let prefix = model_def
                    .whisper_prefix()
                    .ok_or_else(|| format!("Invalid whisper model name: {}", model_def.name))?;
                let encoder_path = model_subdir.join(format!("{}-encoder.onnx", prefix));
                let decoder_path = model_subdir.join(format!("{}-decoder.onnx", prefix));
                let tokens_path = model_subdir.join(format!("{}-tokens.txt", prefix));

                if !encoder_path.exists() {
                    return Err(format!(
                        "Whisper encoder not found at: {}",
                        encoder_path.display()
                    ));
                }
                if !decoder_path.exists() {
                    return Err(format!(
                        "Whisper decoder not found at: {}",
                        decoder_path.display()
                    ));
                }

                let whisper_lang = match language {
                    Some(lang) if lang != "auto" => {
                        let base = lang.split('-').next().unwrap_or(lang);
                        Some(base.to_string())
                    }
                    _ => None,
                };

                info!(
                    engine = "whisper",
                    encoder = %encoder_path.display(),
                    decoder = %decoder_path.display(),
                    language = ?whisper_lang,
                    threads = num_threads,
                    "loading_model"
                );

                config.model_config.whisper = OfflineWhisperModelConfig {
                    encoder: Some(
                        encoder_path
                            .to_str()
                            .ok_or("Invalid encoder path encoding")?
                            .to_string(),
                    ),
                    decoder: Some(
                        decoder_path
                            .to_str()
                            .ok_or("Invalid decoder path encoding")?
                            .to_string(),
                    ),
                    language: whisper_lang,
                    task: Some("transcribe".to_string()),
                    tail_paddings: -1,
                    enable_token_timestamps: false,
                    enable_segment_timestamps: false,
                };
                config.model_config.tokens = Some(
                    tokens_path
                        .to_str()
                        .ok_or("Invalid tokens path encoding")?
                        .to_string(),
                );
            }
            EngineType::Qwen3Asr => {
                let conv_frontend_path = model_subdir.join("conv_frontend.onnx");
                let encoder_path = model_subdir.join("encoder.int8.onnx");
                let decoder_path = model_subdir.join("decoder.int8.onnx");
                let tokenizer_path = model_subdir.join("tokenizer");

                if !conv_frontend_path.exists() {
                    return Err(format!(
                        "Qwen3-ASR conv frontend not found at: {}",
                        conv_frontend_path.display()
                    ));
                }
                if !encoder_path.exists() {
                    return Err(format!(
                        "Qwen3-ASR encoder not found at: {}",
                        encoder_path.display()
                    ));
                }
                if !decoder_path.exists() {
                    return Err(format!(
                        "Qwen3-ASR decoder not found at: {}",
                        decoder_path.display()
                    ));
                }
                if !tokenizer_path.exists() {
                    return Err(format!(
                        "Qwen3-ASR tokenizer not found at: {}",
                        tokenizer_path.display()
                    ));
                }

                info!(
                    engine = "qwen3-asr",
                    conv_frontend = %conv_frontend_path.display(),
                    encoder = %encoder_path.display(),
                    decoder = %decoder_path.display(),
                    tokenizer = %tokenizer_path.display(),
                    threads = num_threads,
                    "loading_model"
                );

                config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
                    conv_frontend: Some(
                        conv_frontend_path
                            .to_str()
                            .ok_or("Invalid conv frontend path encoding")?
                            .to_string(),
                    ),
                    encoder: Some(
                        encoder_path
                            .to_str()
                            .ok_or("Invalid encoder path encoding")?
                            .to_string(),
                    ),
                    decoder: Some(
                        decoder_path
                            .to_str()
                            .ok_or("Invalid decoder path encoding")?
                            .to_string(),
                    ),
                    tokenizer: Some(
                        tokenizer_path
                            .to_str()
                            .ok_or("Invalid tokenizer path encoding")?
                            .to_string(),
                    ),
                    max_total_len: 512,
                    max_new_tokens: 512,
                    temperature: 1e-6,
                    top_p: 0.8,
                    seed: 42,
                    hotwords: None,
                };
            }
            EngineType::Cloud => {
                return Err("Cloud engine not supported by SherpaOnnxEngine".to_string());
            }
        }

        config.model_config.num_threads = num_threads;
        config.model_config.provider = Some(provider.as_str().to_string());
        info!(
            engine = model_def.engine_type.as_str(),
            provider = %provider,
            "provider_configured"
        );
        config.model_config.debug = false;

        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            format!(
                "Failed to create {} recognizer",
                model_def.engine_type.as_str()
            )
        })?;

        info!(
            engine = model_def.engine_type.as_str(),
            model = %model_def.name,
            "model_loaded"
        );

        Ok(Self {
            recognizer: Arc::new(Mutex::new(ThreadSafeRecognizer(recognizer))),
            engine_type: model_def.engine_type,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: usize = 16_000;

    #[test]
    fn short_whisper_audio_is_decoded_once_without_copying_or_truncation() {
        let samples = vec![0.25; SAMPLE_RATE * 20];
        let mut decoded_lengths = Vec::new();

        let text = decode_audio_with(&samples, EngineType::Whisper, |segment| {
            decoded_lengths.push(segment.len());
            Ok("complete short text".to_string())
        })
        .expect("short Whisper decode should succeed");

        assert_eq!(decoded_lengths, vec![samples.len()]);
        assert_eq!(text, "complete short text");
    }

    #[test]
    fn long_whisper_audio_decodes_every_sample_in_order() {
        let samples: Vec<f32> = (0..SAMPLE_RATE * 65)
            .map(|index| (index % 997) as f32 / 997.0)
            .collect();
        let mut recovered = Vec::with_capacity(samples.len());
        let mut decoded_lengths = Vec::new();

        let text = decode_audio_with(&samples, EngineType::Whisper, |segment| {
            recovered.extend_from_slice(segment);
            decoded_lengths.push(segment.len());
            Ok(format!("segment {}", decoded_lengths.len()))
        })
        .expect("long Whisper decode should succeed");

        assert_eq!(recovered, samples);
        assert_eq!(decoded_lengths.len(), 3);
        assert!(decoded_lengths
            .iter()
            .all(|&length| length <= WHISPER_MAX_SEGMENT_SAMPLES));
        assert_eq!(text, "segment 1 segment 2 segment 3");
    }

    #[test]
    fn whisper_ranges_hold_at_and_around_window_boundaries() {
        let sample_counts = [
            WHISPER_MAX_SEGMENT_SAMPLES,
            WHISPER_MAX_SEGMENT_SAMPLES + 1,
            WHISPER_MAX_SEGMENT_SAMPLES * 2 - 1,
            WHISPER_MAX_SEGMENT_SAMPLES * 2,
            WHISPER_MAX_SEGMENT_SAMPLES * 2 + 1,
            SAMPLE_RATE * 121,
        ];

        for sample_count in sample_counts {
            let samples = vec![0.0; sample_count];
            let ranges = transcription_segment_ranges(&samples, EngineType::Whisper);

            assert_eq!(
                ranges.len(),
                sample_count.div_ceil(WHISPER_MAX_SEGMENT_SAMPLES)
            );
            assert_eq!(ranges.first().map(|range| range.start), Some(0));
            assert_eq!(ranges.last().map(|range| range.end), Some(sample_count));
            assert!(ranges
                .iter()
                .all(|range| { !range.is_empty() && range.len() <= WHISPER_MAX_SEGMENT_SAMPLES }));
            assert!(ranges
                .windows(2)
                .all(|neighbors| neighbors[0].end == neighbors[1].start));
        }
    }

    #[test]
    fn whisper_segment_boundary_prefers_nearby_low_energy_audio() {
        let mut samples = vec![0.8; SAMPLE_RATE * 50];
        let silence_start = SAMPLE_RATE * 24 + SAMPLE_RATE * 3 / 4;
        let silence_end = SAMPLE_RATE * 25 + SAMPLE_RATE / 4;
        samples[silence_start..silence_end].fill(0.0);

        let ranges = transcription_segment_ranges(&samples, EngineType::Whisper);

        assert_eq!(ranges.len(), 2);
        assert!(
            (silence_start..silence_end).contains(&ranges[0].end),
            "boundary {} should fall in {}..{}",
            ranges[0].end,
            silence_start,
            silence_end
        );
        assert_eq!(ranges[0].end, ranges[1].start);
        assert_eq!(ranges[1].end, samples.len());
    }

    #[test]
    fn non_whisper_audio_keeps_single_decode_behavior() {
        let samples = vec![0.25; SAMPLE_RATE * 65];

        for engine_type in [EngineType::SenseVoice, EngineType::Qwen3Asr] {
            let mut decoded_lengths = Vec::new();
            decode_audio_with(&samples, engine_type, |segment| {
                decoded_lengths.push(segment.len());
                Ok("complete text".to_string())
            })
            .expect("non-Whisper decode should succeed");

            assert_eq!(decoded_lengths, vec![samples.len()]);
        }
    }

    #[test]
    fn segmented_decode_omits_empty_text_and_propagates_errors() {
        let samples = vec![0.25; SAMPLE_RATE * 65];
        let mut segment_number = 0;

        let text = decode_audio_with(&samples, EngineType::Whisper, |_| {
            segment_number += 1;
            Ok(match segment_number {
                1 => String::new(),
                2 => "middle".to_string(),
                _ => "end".to_string(),
            })
        })
        .expect("empty segment text should not fail the transcription");

        assert_eq!(text, "middle end");

        let mut segment_number = 0;
        let error = decode_audio_with(&samples, EngineType::Whisper, |_| {
            segment_number += 1;
            if segment_number == 2 {
                Err("segment decode failed".to_string())
            } else {
                Ok("partial".to_string())
            }
        })
        .expect_err("one failed segment must fail the complete transcription");

        assert_eq!(segment_number, 2);
        assert_eq!(error, "segment decode failed");
    }
}

impl SherpaOnnxEngine {
    pub fn engine_type(&self) -> EngineType {
        self.engine_type
    }

    pub async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResult, String> {
        let engine_type = self.engine_type();
        let start = std::time::Instant::now();

        let samples = request.samples.clone();
        let engine = self.clone();

        let (text, preprocess_ms, inference_ms) =
            tokio::task::spawn_blocking(move || -> Result<(String, u64, u64), String> {
                let preprocess_start = std::time::Instant::now();

                let duration = samples.len() as f32 / 16_000.0;
                if duration < 0.35 {
                    debug!(
                        engine = engine_type.as_str(),
                        duration_secs = format!("{:.2}", duration),
                        "audio_too_short"
                    );
                    return Ok((String::new(), 0, 0));
                }

                let audio = samples;

                let duration = audio.len() as f32 / 16_000.0;

                let preprocess_ms = preprocess_start.elapsed().as_millis() as u64;
                let inference_start = std::time::Instant::now();

                let guard = engine.recognizer.lock().unwrap();
                let recognizer = &guard.0;
                let text = decode_audio_with(&audio, engine_type, |segment| {
                    let stream = recognizer.create_stream();
                    stream.accept_waveform(AUDIO_SAMPLE_RATE as i32, segment);
                    recognizer.decode(&stream);

                    Ok(stream
                        .get_result()
                        .map(|result| result.text)
                        .unwrap_or_default())
                })?;
                drop(guard);

                let inference_ms = inference_start.elapsed().as_millis() as u64;

                info!(
                    engine = engine_type.as_str(),
                    chars = text.len(),
                    duration_secs = format!("{:.2}", duration),
                    "transcription_completed"
                );

                Ok((text, preprocess_ms, inference_ms))
            })
            .await
            .map_err(|e| format!("Transcription task failed: {}", e))??;

        let total_ms = start.elapsed().as_millis() as u64;

        Ok(TranscriptionResult::with_metrics(
            text,
            engine_type,
            total_ms,
            Some(0),
            Some(preprocess_ms),
            Some(inference_ms),
        ))
    }
}
